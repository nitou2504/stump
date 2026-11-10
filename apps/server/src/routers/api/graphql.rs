use crate::middleware::{auth::auth_middleware, host::HostExtractor};
use async_graphql::http::{Credentials, GraphiQLSource, ALL_WEBSOCKET_PROTOCOLS};
use async_graphql_axum::{
	GraphQLProtocol, GraphQLRequest, GraphQLResponse, GraphQLWebSocket,
};
use axum::{
	extract::{ws::WebSocketUpgrade, State},
	middleware,
	response::{Html, IntoResponse},
	routing::{get, post},
	Extension, Router,
};

use graphql::{
	data::{AuthContext, EdgeRequest, ServiceContext},
	schema::{build_schema, AppSchema},
};
use tower_sessions::Session;

use crate::{config::state::AppState, errors::APIError};

pub(crate) async fn mount(app_state: AppState) -> Router<AppState> {
	let schema = build_schema(app_state.clone()).await;

	let mut method_router = post(graphql_handler);
	if app_state.config.enable_playground || cfg!(debug_assertions) {
		method_router = method_router.get(playground);
	}

	Router::new()
		.route("/", method_router)
		.route("/ws", get(graphql_subscription_handler))
		.layer(middleware::from_fn_with_state(app_state, auth_middleware))
		.layer(Extension(schema))
}

// TODO: Consider new user permission
async fn playground(
	Extension(req_ctx): Extension<AuthContext>,
) -> Result<impl IntoResponse, APIError> {
	if !req_ctx.user().is_server_owner {
		return Err(APIError::forbidden_discreet());
	}

	Ok(Html(
		GraphiQLSource::build()
			.endpoint("/api/graphql")
			.subscription_endpoint("/api/graphql/ws")
			.credentials(Credentials::Include)
			.finish(),
	))
}

/// The only fields a user who has to choose their own password can ask for: who they are and the
/// mutation that changes the password. Anything else (a fragment at the root, a subscription, any
/// other field) is refused
const FIELDS_WHILE_PASSWORD_MUST_CHANGE: [&str; 3] = ["me", "updateViewer", "__typename"];

fn asks_only_for_the_password_change(query: &str) -> bool {
	use async_graphql::parser::{
		parse_query,
		types::{DocumentOperations, OperationDefinition, OperationType, Selection},
	};

	let Ok(document) = parse_query(query) else {
		return false;
	};
	let operations: Vec<&OperationDefinition> = match &document.operations {
		DocumentOperations::Single(operation) => vec![&operation.node],
		DocumentOperations::Multiple(operations) => {
			operations.values().map(|operation| &operation.node).collect()
		},
	};
	operations.iter().all(|operation| {
		operation.ty != OperationType::Subscription
			&& operation.selection_set.node.items.iter().all(|selection| {
				matches!(
					&selection.node,
					Selection::Field(field)
						if FIELDS_WHILE_PASSWORD_MUST_CHANGE.contains(&field.node.name.node.as_str())
				)
			})
	})
}

async fn graphql_handler(
	schema: Extension<AppSchema>,
	Extension(auth): Extension<AuthContext>,
	Extension(edge): Extension<EdgeRequest>,
	HostExtractor(details): HostExtractor,
	session: Session,
	req: GraphQLRequest,
) -> GraphQLResponse {
	let mut req = req.into_inner();

	// the owner set this user's password: until they choose their own, all they may do here is
	// ask who they are and change it
	if auth.user.must_change_password && !asks_only_for_the_password_change(&req.query) {
		return async_graphql::Response::from_errors(vec![async_graphql::ServerError::new(
			"You must choose a new password before you can use anything else",
			None,
		)])
		.into();
	}

	req = req.data(auth);
	req = req.data(edge);
	req = req.data(ServiceContext {
		host: details.host,
		scheme: details.scheme,
	});
	req = req.data(session);
	schema.execute(req).await.into()
}

async fn graphql_subscription_handler(
	schema: Extension<AppSchema>,
	State(ctx): State<AppState>,
	Extension(auth): Extension<AuthContext>,
	Extension(edge): Extension<EdgeRequest>,
	protocol: GraphQLProtocol,
	websocket: WebSocketUpgrade,
) -> impl IntoResponse {
	let mut data = async_graphql::Data::default();
	data.insert(auth);
	// the websocket runs queries and mutations too: without this the guards that read it would
	// not know how the request arrived (and refuse, but a handler must not rely on that)
	data.insert(edge);
	data.insert(ctx);

	websocket
		.protocols(ALL_WEBSOCKET_PROTOCOLS)
		.on_upgrade(move |stream| {
			GraphQLWebSocket::new(stream, schema.0, protocol)
				.with_data(data)
				.serve()
		})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_user_who_must_change_the_password_may_ask_for_two_things() {
		for query in [
			"{ me { id } }",
			"{ __typename }",
			"mutation { updateViewer(input: {username: \"a\", password: \"b\"}) { id } }",
			"query Me { me { username } }",
		] {
			assert!(asks_only_for_the_password_change(query), "{query}");
		}
		for query in [
			"{ libraries { nodes { id } } }",
			"{ me { id } libraries { nodes { id } } }",
			"{ ...F } fragment F on Query { libraries { nodes { id } } }",
			"subscription { readEvents { __typename } }",
			"mutation { deleteLoginActivity }",
			"{ alias: libraries { nodes { id } } }",
			"not a query",
			"",
		] {
			assert!(!asks_only_for_the_password_change(query), "{query}");
		}
	}
}
