/* eslint-disable */
import { DocumentTypeDecoration } from '@graphql-typed-document-node/core';
export type Maybe<T> = T | null;
export type InputMaybe<T> = Maybe<T>;
export type Exact<T extends { [key: string]: unknown }> = { [K in keyof T]: T[K] };
export type MakeOptional<T, K extends keyof T> = Omit<T, K> & { [SubKey in K]?: Maybe<T[SubKey]> };
export type MakeMaybe<T, K extends keyof T> = Omit<T, K> & { [SubKey in K]: Maybe<T[SubKey]> };
export type MakeEmpty<T extends { [key: string]: unknown }, K extends keyof T> = { [_ in K]?: never };
export type Incremental<T> = T | { [P in keyof T]?: P extends ' $fragmentName' | '__typename' ? T[P] : never };
/** All built-in and custom scalars, mapped to their actual values */
export type Scalars = {
  ID: { input: string; output: string; }
  String: { input: string; output: string; }
  Boolean: { input: boolean; output: boolean; }
  Int: { input: number; output: number; }
  Float: { input: number; output: number; }
  /**
   * Implement the DateTime<FixedOffset> scalar
   *
   * The input/output is a string in RFC3339 format.
   */
  DateTime: { input: any; output: any; }
  Decimal: { input: any; output: any; }
  /** A scalar that can represent any JSON value. */
  JSON: { input: any; output: any; }
  /** A scalar that can represent any JSON Object value. */
  JSONObject: { input: any; output: any; }
  /**
   * ISO 8601 calendar date without timezone.
   * Format: %Y-%m-%d
   *
   * # Examples
   *
   * * `1994-11-13`
   * * `2000-02-24`
   */
  NaiveDate: { input: any; output: any; }
  /** A multipart file upload */
  Upload: { input: any; output: any; }
};

/**
 * The different roles a user may have for a role-based access control system scoped
 * to a specific entity
 */
export enum AccessRole {
  CoCreator = 'CO_CREATOR',
  Reader = 'READER',
  Writer = 'WRITER'
}

export type AddBookToClubInput = {
  book: BookClubBookInput;
};

export type AgeRestriction = {
  __typename?: 'AgeRestriction';
  age: Scalars['Int']['output'];
  id: Scalars['Int']['output'];
  restrictOnUnset: Scalars['Boolean']['output'];
  userId: Scalars['String']['output'];
};

export type AgeRestrictionInput = {
  age: Scalars['Int']['input'];
  restrictOnUnset: Scalars['Boolean']['input'];
};

export type AggregatedReaction = {
  __typename?: 'AggregatedReaction';
  count: Scalars['Int']['output'];
  customEmojiId?: Maybe<Scalars['Int']['output']>;
  customEmojiUrl?: Maybe<Scalars['String']['output']>;
  emoji?: Maybe<Scalars['String']['output']>;
  reactedByMe: Scalars['Boolean']['output'];
};

export type AnalyzeMediaOutput = {
  __typename?: 'AnalyzeMediaOutput';
  /** The number of media item updates performed */
  mediaUpdated: Scalars['Int']['output'];
  /** The number of pages in total that were analyzed to some extent */
  pagesAnalyzed: Scalars['Int']['output'];
};

/**
 * How an annotation is drawn over its text in a reader, like KOReader's highlight styles.
 * Each style can carry a note.
 */
export enum AnnotationStyle {
  /** A coloured background behind the text */
  Highlight = 'HIGHLIGHT',
  /** The text and background colours swapped */
  Invert = 'INVERT',
  Strikethrough = 'STRIKETHROUGH',
  Underline = 'UNDERLINE'
}

export type Apikey = {
  __typename?: 'Apikey';
  createdAt: Scalars['DateTime']['output'];
  expiresAt?: Maybe<Scalars['DateTime']['output']>;
  id: Scalars['Int']['output'];
  lastUsedAt?: Maybe<Scalars['DateTime']['output']>;
  longTokenHash: Scalars['String']['output'];
  name: Scalars['String']['output'];
  permissions: ApikeyPermissionsOutput;
  shortToken: Scalars['String']['output'];
  userId: Scalars['String']['output'];
};

export type ApikeyInput = {
  /** The expiration date for the API key, if any */
  expiresAt?: InputMaybe<Scalars['DateTime']['input']>;
  /** The name of the API key */
  name: Scalars['String']['input'];
  /** The permissions that the API key should have */
  permissions: ApikeyPermissions;
};

export type ApikeyPermissions =
  { custom: Array<UserPermission>; inherit?: never; }
  |  { custom?: never; inherit: InheritPermissionValue; };

export type ApikeyPermissionsOutput = InheritPermissionStruct | UserPermissionStruct;

export type Arrangement = {
  __typename?: 'Arrangement';
  locked: Scalars['Boolean']['output'];
  sections: Array<ArrangementSection>;
};

export type ArrangementConfig = CustomArrangementConfig | InProgressBooks | OnDeckBooks | RecentlyAdded | SystemArrangementConfig;

export type ArrangementConfigInput =
  { custom: FilterableArrangementEntityLinkInput; inProgressBooks?: never; onDeckBooks?: never; recentlyAdded?: never; system?: never; }
  |  { custom?: never; inProgressBooks: InProgressBooksInput; onDeckBooks?: never; recentlyAdded?: never; system?: never; }
  |  { custom?: never; inProgressBooks?: never; onDeckBooks: OnDeckBooksInput; recentlyAdded?: never; system?: never; }
  |  { custom?: never; inProgressBooks?: never; onDeckBooks?: never; recentlyAdded: RecentlyAddedInput; system?: never; }
  |  { custom?: never; inProgressBooks?: never; onDeckBooks?: never; recentlyAdded?: never; system: SystemArrangementConfigInput; };

export type ArrangementSection = {
  __typename?: 'ArrangementSection';
  config: ArrangementConfig;
  visible: Scalars['Boolean']['output'];
};

export type ArrangementSectionInput = {
  config: ArrangementConfigInput;
  visible?: Scalars['Boolean']['input'];
};

export type AttachmentMeta = {
  __typename?: 'AttachmentMeta';
  /** The filename of the attachment */
  filename: Scalars['String']['output'];
  media?: Maybe<Media>;
  /** The associated media ID of the attachment, if there is one */
  mediaId?: Maybe<Scalars['String']['output']>;
  /** The size of the attachment in bytes */
  size: Scalars['Int']['output'];
};

export type Author = {
  __typename?: 'Author';
  books: Array<Media>;
  name: Scalars['String']['output'];
  /**
   * The role of this author relative to the context they are queried in (e.g., a series).
   * This field will be None when queried outside of a context in which an author has a role,
   * like at a library-level query
   */
  role?: Maybe<AuthorRole>;
  series: Array<AuthorSeries>;
  /** Books where this author shares credit with other writers (co-authored works) */
  sharedWorks: Array<SharedWork>;
  /** Books where this author is the sole credited writer (no co-authors) */
  standalones: Array<Media>;
};

/** The role of an author in relation to a work or series */
export enum AuthorRole {
  CoAuthor = 'CO_AUTHOR',
  Primary = 'PRIMARY'
}

export type AuthorSeries = {
  __typename?: 'AuthorSeries';
  /** Authors who contributed to this series */
  authors: Array<Author>;
  books: Array<Media>;
  title: Scalars['String']['output'];
};

export type BookClub = {
  __typename?: 'BookClub';
  /** All books in the club's queue, ordered by position */
  books: Array<BookClubBook>;
  createdAt: Scalars['DateTime']['output'];
  creator: BookClubMember;
  /** The current book being read */
  currentBook?: Maybe<BookClubBook>;
  description?: Maybe<Scalars['String']['output']>;
  emoji?: Maybe<Scalars['String']['output']>;
  id: Scalars['String']['output'];
  invitations: Array<BookClubInvitation>;
  isPrivate: Scalars['Boolean']['output'];
  members: Array<BookClubMember>;
  membersCount: Scalars['Int']['output'];
  membership?: Maybe<BookClubMember>;
  moderators: Array<BookClubMember>;
  name: Scalars['String']['output'];
  /** Get discussions that are pinned for this book club */
  pinnedDiscussions: Array<BookClubDiscussion>;
  /** The previous book that was read, if it exists */
  previousBook?: Maybe<BookClubBook>;
  /** All previous books that were read, ordered by completion date (most recent first) */
  previousBooks: Array<BookClubBook>;
  previousDiscussionsCount: Scalars['Int']['output'];
  roleSpec: Scalars['JSON']['output'];
  slug: Scalars['String']['output'];
};

export type BookClubBook = {
  __typename?: 'BookClubBook';
  addedAt: Scalars['DateTime']['output'];
  author?: Maybe<Scalars['String']['output']>;
  bookClubId: Scalars['String']['output'];
  bookEntityId?: Maybe<Scalars['String']['output']>;
  completedAt?: Maybe<Scalars['DateTime']['output']>;
  discussions: Array<BookClubDiscussion>;
  entity?: Maybe<Media>;
  id: Scalars['String']['output'];
  imageUrl?: Maybe<Scalars['String']['output']>;
  position: Scalars['Int']['output'];
  title?: Maybe<Scalars['String']['output']>;
  url?: Maybe<Scalars['String']['output']>;
};

export type BookClubBookInput =
  { external: BookClubExternalBookInput; stored?: never; }
  |  { external?: never; stored: BookClubInternalBookInput; };

export type BookClubBookSuggestion = {
  __typename?: 'BookClubBookSuggestion';
  author?: Maybe<Scalars['String']['output']>;
  bookClubId: Scalars['String']['output'];
  bookId?: Maybe<Scalars['String']['output']>;
  createdAt: Scalars['DateTime']['output'];
  id: Scalars['String']['output'];
  /** Check if the current user has liked this suggestion */
  isLikedByMe: Scalars['Boolean']['output'];
  /**
   * Get the count of likes (votes) on this suggestion
   * TODO(dataloader): Create dataloader
   */
  likeCount: Scalars['Int']['output'];
  notes?: Maybe<Scalars['String']['output']>;
  resolvedAt?: Maybe<Scalars['DateTime']['output']>;
  /** Get the member who resolved this suggestion */
  resolvedBy?: Maybe<BookClubMember>;
  resolvedById?: Maybe<Scalars['String']['output']>;
  status: BookClubSuggestionStatus;
  /** Get the member who suggested this book */
  suggestedBy: BookClubMember;
  suggestedById: Scalars['String']['output'];
  title?: Maybe<Scalars['String']['output']>;
  url?: Maybe<Scalars['String']['output']>;
};

export type BookClubDiscussion = {
  __typename?: 'BookClubDiscussion';
  /** Get the book this discussion is for */
  book?: Maybe<BookClubBook>;
  bookClubBookId?: Maybe<Scalars['String']['output']>;
  bookClubId: Scalars['String']['output'];
  createdAt: Scalars['DateTime']['output'];
  /** A display name for the discussion */
  displayName: Scalars['String']['output'];
  emoji?: Maybe<Scalars['String']['output']>;
  id: Scalars['String']['output'];
  isArchived: Scalars['Boolean']['output'];
  isLocked: Scalars['Boolean']['output'];
  isPinned: Scalars['Boolean']['output'];
  /**
   * Get the count of messages in this discussion (excluding deleted messages)
   * TODO(dataloader): Create dataloader
   */
  messageCount: Scalars['Int']['output'];
  title?: Maybe<Scalars['String']['output']>;
};

export type BookClubDiscussionInput = {
  bookClubBookId?: InputMaybe<Scalars['ID']['input']>;
  isPinned: Scalars['Boolean']['input'];
  title?: InputMaybe<Scalars['String']['input']>;
};

export type BookClubDiscussionMessage = {
  __typename?: 'BookClubDiscussionMessage';
  bookClubId: Scalars['String']['output'];
  content: Scalars['String']['output'];
  deletedAt?: Maybe<Scalars['String']['output']>;
  discussionId: Scalars['String']['output'];
  editedAt?: Maybe<Scalars['DateTime']['output']>;
  id: Scalars['String']['output'];
  isPinnedMessage: Scalars['Boolean']['output'];
  /** Get the member who posted this message */
  member?: Maybe<BookClubMember>;
  memberId?: Maybe<Scalars['String']['output']>;
  parentMessageId?: Maybe<Scalars['String']['output']>;
  /**
   * Get aggregated reactions for this message, grouped by emoji, sorted by count desc
   * TODO(dataloader): Create dataloader
   */
  reactions: Array<AggregatedReaction>;
  /** Get the message this message is an inline reply to (if any) */
  replyTo?: Maybe<BookClubDiscussionMessage>;
  replyToMessageId?: Maybe<Scalars['String']['output']>;
  /**
   * Get the threaded replies to this message (if any)
   * TODO(dataloader): Create dataloader
   */
  threadChildren: Array<BookClubDiscussionMessage>;
  /**
   * Get the count of threaded replies to this message (if any)
   * TODO(dataloader): Create dataloader
   */
  threadChildrenCount: Scalars['Int']['output'];
  timestamp: Scalars['DateTime']['output'];
};

export type BookClubExternalBookInput = {
  author: Scalars['String']['input'];
  imageUrl?: InputMaybe<Scalars['String']['input']>;
  title: Scalars['String']['input'];
  url?: InputMaybe<Scalars['String']['input']>;
};

export type BookClubInternalBookInput = {
  id: Scalars['String']['input'];
};

export type BookClubInvitation = {
  __typename?: 'BookClubInvitation';
  /** The book club that the user was invited to */
  bookClub: BookClub;
  bookClubId: Scalars['String']['output'];
  id: Scalars['String']['output'];
  role: BookClubMemberRole;
  /** The user who was invited to the book club */
  user: User;
  userId: Scalars['String']['output'];
};

export type BookClubInvitationInput = {
  role?: InputMaybe<BookClubMemberRole>;
  userId: Scalars['String']['input'];
};

export type BookClubInvitationResponseInput = {
  accept: Scalars['Boolean']['input'];
  member?: InputMaybe<BookClubMemberInput>;
};

export type BookClubMember = {
  __typename?: 'BookClubMember';
  avatarUrl?: Maybe<Scalars['String']['output']>;
  bio?: Maybe<Scalars['String']['output']>;
  bookClubId: Scalars['String']['output'];
  displayName?: Maybe<Scalars['String']['output']>;
  hideProgress: Scalars['Boolean']['output'];
  id: Scalars['String']['output'];
  isCreator: Scalars['Boolean']['output'];
  joinedAt: Scalars['DateTime']['output'];
  role: BookClubMemberRole;
  user: User;
  userId: Scalars['String']['output'];
  username: Scalars['String']['output'];
};

export type BookClubMemberInput = {
  displayName?: InputMaybe<Scalars['String']['input']>;
  userId: Scalars['String']['input'];
};

/** The role of a member within a book club */
export enum BookClubMemberRole {
  Admin = 'ADMIN',
  Creator = 'CREATOR',
  Member = 'MEMBER',
  Moderator = 'MODERATOR'
}

/** The status of a book suggestion */
export enum BookClubSuggestionStatus {
  Accepted = 'ACCEPTED',
  Pending = 'PENDING',
  Rejected = 'REJECTED'
}

/** The full timeline of events for a book, including all readthroughs and their sessions */
export type BookReadingTimeline = {
  __typename?: 'BookReadingTimeline';
  readthroughs: Array<ReadthroughTimeline>;
  totalElapsedSeconds: Scalars['Int']['output'];
};

export type Bookmark = {
  __typename?: 'Bookmark';
  createdAt: Scalars['DateTime']['output'];
  id: Scalars['String']['output'];
  locator?: Maybe<ReadiumLocator>;
  mediaId: Scalars['String']['output'];
  page?: Maybe<Scalars['Int']['output']>;
  previewContent?: Maybe<Scalars['String']['output']>;
  /**
   * The ID of the session which this bookmark was created in. Realistically, when using a Stump
   * reader this should always be set
   */
  sessionId?: Maybe<Scalars['Int']['output']>;
  userId: Scalars['String']['output'];
};

export type BookmarkInput = {
  locator: ReadiumLocatorInput;
  mediaId: Scalars['String']['input'];
  previewContent?: InputMaybe<Scalars['String']['input']>;
};

export type CleanLibraryResponse = {
  __typename?: 'CleanLibraryResponse';
  deletedMediaCount: Scalars['Int']['output'];
  deletedSeriesCount: Scalars['Int']['output'];
  isEmpty: Scalars['Boolean']['output'];
};

/**
 * Represents a collected issue/series within a TPB or GN
 * See https://github.com/mylar3/mylar3/wiki/series.json-schema-%28version-1.0.1%29
 */
export type CollectedItem = {
  __typename?: 'CollectedItem';
  /** CV ComicID of series */
  comicid?: Maybe<Scalars['String']['output']>;
  /** CV IssueID of single issue (not valid if multiple issues) */
  issueid?: Maybe<Scalars['String']['output']>;
  /** Listing of issue numbers present pertaining to related comicid in collection */
  issues?: Maybe<Scalars['String']['output']>;
  /** The title of the series */
  series?: Maybe<Scalars['String']['output']>;
};

/**
 * Represents a collected issue/series within a TPB or GN
 * See https://github.com/mylar3/mylar3/wiki/series.json-schema-%28version-1.0.1%29
 */
export type CollectedItemInput = {
  /** CV ComicID of series */
  comicid?: InputMaybe<Scalars['String']['input']>;
  /** CV IssueID of single issue (not valid if multiple issues) */
  issueid?: InputMaybe<Scalars['String']['input']>;
  /** Listing of issue numbers present pertaining to related comicid in collection */
  issues?: InputMaybe<Scalars['String']['input']>;
  /** The title of the series */
  series?: InputMaybe<Scalars['String']['input']>;
};

export type ComputedFilterLibraryType =
  { is: LibraryType; isAnyOf?: never; isNoneOf?: never; isNot?: never; }
  |  { is?: never; isAnyOf: Array<LibraryType>; isNoneOf?: never; isNot?: never; }
  |  { is?: never; isAnyOf?: never; isNoneOf: Array<LibraryType>; isNot?: never; }
  |  { is?: never; isAnyOf?: never; isNoneOf?: never; isNot: LibraryType; };

export type ComputedFilterReadingStatus =
  { is: ReadingStatus; isAnyOf?: never; isNoneOf?: never; isNot?: never; }
  |  { is?: never; isAnyOf: Array<ReadingStatus>; isNoneOf?: never; isNot?: never; }
  |  { is?: never; isAnyOf?: never; isNoneOf: Array<ReadingStatus>; isNot?: never; }
  |  { is?: never; isAnyOf?: never; isNoneOf?: never; isNot: ReadingStatus; };

/** A factor that contributed to a match's confidence score */
export type ConfidenceFactor = {
  __typename?: 'ConfidenceFactor';
  /** Name of the scoring factor (e.g., "title_exact_match") */
  factor: Scalars['String']['output'];
  /** Whether this factor matched */
  matched: Scalars['Boolean']['output'];
  /** How much weight this factor carried */
  weight: Scalars['Float']['output'];
};

/** An event that is emitted by the core and consumed by a client */
export type CoreEvent = CreatedManySeries | CreatedMedia | CreatedOrUpdatedManyMedia | DiscoveredMissingLibrary | JobOutput | JobStarted | JobUpdate;

export type CoreJobOutput = AnalyzeMediaOutput | LibraryScanOutput | MetadataFetchJobOutput | PlaceholderGenerationOutput | PostProcessOutput | RescanBooksOutput | SeriesScanOutput | ThumbnailGenerationOutput | TrashPurgeOutput;

export type CreateAnnotationInput = {
  annotationText?: InputMaybe<Scalars['String']['input']>;
  locator: ReadiumLocatorInput;
  mediaId: Scalars['String']['input'];
  style?: AnnotationStyle;
};

export type CreateBookClubInput = {
  creatorDisplayName?: InputMaybe<Scalars['String']['input']>;
  creatorHideProgress: Scalars['Boolean']['input'];
  description?: InputMaybe<Scalars['String']['input']>;
  isPrivate?: Scalars['Boolean']['input'];
  memberRoleSpec?: InputMaybe<Scalars['JSON']['input']>;
  name: Scalars['String']['input'];
  slug?: InputMaybe<Scalars['String']['input']>;
};

export type CreateBookClubMemberInput = {
  displayName?: InputMaybe<Scalars['String']['input']>;
  role: BookClubMemberRole;
  userId: Scalars['String']['input'];
};

export type CreateCustomEmojiInput = {
  isAnimated: Scalars['Boolean']['input'];
  name: Scalars['String']['input'];
};

/** Input object for creating a metadata provider configuration */
export type CreateMetadataProviderConfigInput = {
  /** The API token for authenticating with the provider */
  apiToken: Scalars['String']['input'];
  /**
   * Optional expiration date for the API key. This is exclusively a QOL thing,
   * since the creds don't live within the management domain of Stump
   */
  apiTokenExpiresAt?: InputMaybe<Scalars['DateTime']['input']>;
  /** Auto-apply configuration */
  autoApplyConfig?: InputMaybe<Scalars['JSON']['input']>;
  /** Whether the provider is enabled */
  enabled?: InputMaybe<Scalars['Boolean']['input']>;
  /** The provider type */
  providerType: MetadataProvider;
};

export type CreateOrUpdateLibraryInput = {
  config?: InputMaybe<LibraryConfigInput>;
  description?: InputMaybe<Scalars['String']['input']>;
  emoji?: InputMaybe<Scalars['String']['input']>;
  name: Scalars['String']['input'];
  path: Scalars['String']['input'];
  scanAfterPersist?: Scalars['Boolean']['input'];
  tags?: InputMaybe<Array<Scalars['String']['input']>>;
};

export type CreateScheduledJobInput = {
  /** The type-specific config. The kind is inferred from the variant provided */
  config: ScheduledJobConfigInput;
  /** Whether the job is enabled. Defaults to `true` */
  enabled?: InputMaybe<Scalars['Boolean']['input']>;
  name: Scalars['String']['input'];
  /** A cron expression (e.g. `0 0 * * *` for daily at midnight) */
  schedule: Scalars['String']['input'];
};

export type CreateUserInput = {
  ageRestriction?: InputMaybe<AgeRestrictionInput>;
  maxSessionsAllowed?: InputMaybe<Scalars['Int']['input']>;
  password: Scalars['String']['input'];
  permissions: Array<UserPermission>;
  username: Scalars['String']['input'];
};

export type CreatedApiKey = {
  __typename?: 'CreatedAPIKey';
  apiKey: Apikey;
  secret: Scalars['String']['output'];
};

export type CreatedManySeries = {
  __typename?: 'CreatedManySeries';
  count: Scalars['Int']['output'];
  libraryId: Scalars['String']['output'];
};

export type CreatedMedia = {
  __typename?: 'CreatedMedia';
  id: Scalars['String']['output'];
  libraryId: Scalars['String']['output'];
  seriesId?: Maybe<Scalars['String']['output']>;
};

export type CreatedOrUpdatedManyMedia = {
  __typename?: 'CreatedOrUpdatedManyMedia';
  count: Scalars['Int']['output'];
  libraryId: Scalars['String']['output'];
  seriesId?: Maybe<Scalars['String']['output']>;
};

/** How well one of a book's current tags fits it */
export type CurrentGenreFit = {
  __typename?: 'CurrentGenreFit';
  /** The evidence for it besides the book itself, 0-1 */
  confidence: Scalars['Float']['output'];
  /** Whether anything besides the book supports it (facets always fit) */
  fits: Scalars['Boolean']['output'];
  /** `GENRE` or `FACET` */
  kind: Scalars['String']['output'];
  /** How many books of the library carry it */
  libraryCount: Scalars['Int']['output'];
  reasons: Array<Scalars['String']['output']>;
  value: Scalars['String']['output'];
};

export type CursorPaginatedBookClubDiscussionMessageResponse = {
  __typename?: 'CursorPaginatedBookClubDiscussionMessageResponse';
  cursorInfo: CursorPaginationInfo;
  nodes: Array<BookClubDiscussionMessage>;
};

export type CursorPaginatedGlobalReadingTimelineNodeResponse = {
  __typename?: 'CursorPaginatedGlobalReadingTimelineNodeResponse';
  cursorInfo: CursorPaginationInfo;
  nodes: Array<GlobalReadingTimelineNode>;
};

/** A simple cursor-based pagination input object */
export type CursorPagination = {
  after?: InputMaybe<Scalars['String']['input']>;
  limit?: Scalars['Int']['input'];
};

/** Information about the current cursor pagination state */
export type CursorPaginationInfo = {
  __typename?: 'CursorPaginationInfo';
  /**
   * The cursor of the current page. This should only be None if there are no results,
   * since there is no cursor present to pull from. This technically deviates from
   * popular (read: Relay) specs, but it works better for Stump
   */
  currentCursor?: Maybe<Scalars['String']['output']>;
  /** The limit used when querying the database */
  limit: Scalars['Int']['output'];
  /** The cursor the next page should use, if it exists. */
  nextCursor?: Maybe<Scalars['String']['output']>;
};

export type CustomArrangementConfig = {
  __typename?: 'CustomArrangementConfig';
  entity: FilterableArrangementEntity;
  filter?: Maybe<Scalars['JSON']['output']>;
  links: Array<FilterableArrangementEntityLink>;
  name?: Maybe<Scalars['String']['output']>;
  orderBy?: Maybe<Scalars['String']['output']>;
};

export type CustomEmoji = {
  __typename?: 'CustomEmoji';
  createdAt: Scalars['DateTime']['output'];
  createdById: Scalars['String']['output'];
  fileExtension: Scalars['String']['output'];
  id: Scalars['Int']['output'];
  isAnimated: Scalars['Boolean']['output'];
  name: Scalars['String']['output'];
  /** A reference to the URL of the custom emoji's thumbnail. This is not the full image, but a smaller thumbnail version. */
  url: Scalars['String']['output'];
};

/** The reading activity (time, number of unique sessions) for a specific date */
export type DailyActivity = {
  __typename?: 'DailyActivity';
  date: Scalars['NaiveDate']['output'];
  sessionCount: Scalars['Int']['output'];
  totalSeconds: Scalars['Int']['output'];
};

/**
 * An input for specifying a date range:
 * - `start = null` means from the beginning of the user's history
 * - `end = null` resolves to the current logical day (per user prefs)
 */
export type DateRangeInput = {
  end?: InputMaybe<Scalars['NaiveDate']['input']>;
  start?: InputMaybe<Scalars['NaiveDate']['input']>;
};

export type DeleteJobAssociatedLogs = {
  __typename?: 'DeleteJobAssociatedLogs';
  /** The number of logs deleted that were related to a job */
  affectedRows: Scalars['Int']['output'];
};

export type DeleteJobHistory = {
  __typename?: 'DeleteJobHistory';
  /** The number of logs deleted that were related to a job */
  affectedRows: Scalars['Int']['output'];
};

/**
 * A book in *Deleted books*: removed from its library, hidden everywhere, its file still on
 * disk until it is deleted permanently
 */
export type DeletedBook = {
  __typename?: 'DeletedBook';
  /** The book as it was when it was removed (its progress, annotations and tags are kept) */
  book: Media;
  deletedAt: Scalars['DateTime']['output'];
  /** Who removed it, when known */
  deletedBy?: Maybe<Scalars['String']['output']>;
  /** When its time in Deleted books is over (null: kept until deleted by hand) */
  expiresAt?: Maybe<Scalars['DateTime']['output']>;
  /** Whether its file is still there (otherwise deleting it permanently only deletes its data) */
  fileExists: Scalars['Boolean']['output'];
  libraryId: Scalars['String']['output'];
  libraryName: Scalars['String']['output'];
  /** When the automatic purge will delete it permanently */
  purgeAt?: Maybe<Scalars['DateTime']['output']>;
  /** Its path inside its library */
  relativePath: Scalars['String']['output'];
};

/** A book a purge couldn't delete permanently */
export type DeletedBooksPurgeError = {
  __typename?: 'DeletedBooksPurgeError';
  error: Scalars['String']['output'];
  mediaId: Scalars['String']['output'];
  title: Scalars['String']['output'];
};

/** What deleting books permanently did */
export type DeletedBooksPurgeResult = {
  __typename?: 'DeletedBooksPurgeResult';
  /** The size of the book files deleted */
  freedBytes: Scalars['Int']['output'];
  /** Books kept in Deleted books, with why */
  kept: Array<DeletedBooksPurgeError>;
  /** Books whose file was already gone: only their data was deleted */
  missingFiles: Scalars['Int']['output'];
  /** Books deleted permanently: their file and their data */
  purged: Scalars['Int']['output'];
};

/** What an automatic purge of expired deleted books did */
export type DeletedBooksPurgeSummary = {
  __typename?: 'DeletedBooksPurgeSummary';
  /** Why books were kept (the first few) */
  errors: Array<DeletedBooksPurgeError>;
  /** Books kept in Deleted books because their file couldn't be deleted */
  failed: Scalars['Int']['output'];
  /** When it finished */
  finishedAt: Scalars['DateTime']['output'];
  /** The job that ran it, if it ran as a job (nothing had expired otherwise) */
  jobId?: Maybe<Scalars['String']['output']>;
  /** Books whose file was already gone: only their data was deleted */
  missingFiles: Scalars['Int']['output'];
  /** Held originals (and put-back versions) whose time was over, deleted */
  originals: Scalars['Int']['output'];
  /** Books deleted permanently (their file and their data) */
  purged: Scalars['Int']['output'];
};

/** How *Deleted books* works on this server */
export type DeletedBooksSettings = {
  __typename?: 'DeletedBooksSettings';
  heldBytes: Scalars['Int']['output'];
  /** How many files are held there, and their size */
  heldFiles: Scalars['Int']['output'];
  /** What it did (library managers only) */
  lastPurge?: Maybe<DeletedBooksPurgeSummary>;
  /** When the automatic purge last ran (library managers only) */
  lastPurgeAt?: Maybe<Scalars['DateTime']['output']>;
  /** When the automatic purge runs next (null when it is off) */
  nextPurgeAt?: Maybe<Scalars['DateTime']['output']>;
  /**
   * How many days removed books and held files stay in Deleted books (0: until deleted by
   * hand)
   */
  retentionDays: Scalars['Int']['output'];
  /**
   * Stump's trash folder, where the files taken out of a library (originals replaced by
   * post-processing) are held (library managers only)
   */
  trashDir?: Maybe<Scalars['String']['output']>;
};

export enum Dimension {
  Height = 'HEIGHT',
  Width = 'WIDTH'
}

export type DirectoryListing = {
  __typename?: 'DirectoryListing';
  files: Array<DirectoryListingFile>;
  parent?: Maybe<Scalars['String']['output']>;
};

export type DirectoryListingFile = {
  __typename?: 'DirectoryListingFile';
  isDirectory: Scalars['Boolean']['output'];
  media?: Maybe<Media>;
  name: Scalars['String']['output'];
  path: Scalars['String']['output'];
};

export type DirectoryListingInput = {
  ignoreDirectories?: Scalars['Boolean']['input'];
  ignoreFiles?: Scalars['Boolean']['input'];
  ignoreHidden?: Scalars['Boolean']['input'];
  path?: InputMaybe<Scalars['String']['input']>;
};

export type DiscordConfig = {
  __typename?: 'DiscordConfig';
  webhookUrl: Scalars['String']['output'];
};

export type DiscordConfigInput = {
  webhookUrl: Scalars['String']['input'];
};

export type DiscoveredMissingLibrary = {
  __typename?: 'DiscoveredMissingLibrary';
  id: Scalars['String']['output'];
};

export type EditMessageInput = {
  content: Scalars['String']['input'];
};

/** Input object for creating or updating an email device */
export type EmailDeviceInput = {
  /** The email address of the device */
  email: Scalars['String']['input'];
  /** Whether the device is forbidden from receiving emails from the server. */
  forbidden: Scalars['Boolean']['input'];
  /** The friendly name of the email device, e.g. "Aaron's Kobo" */
  name: Scalars['String']['input'];
};

export type Emailer = {
  __typename?: 'Emailer';
  id: Scalars['Int']['output'];
  isPrimary: Scalars['Boolean']['output'];
  lastUsedAt?: Maybe<Scalars['DateTime']['output']>;
  maxAttachmentSizeBytes?: Maybe<Scalars['Int']['output']>;
  maxNumAttachments?: Maybe<Scalars['Int']['output']>;
  name: Scalars['String']['output'];
  sendHistory: Array<EmailerSendRecord>;
  senderDisplayName: Scalars['String']['output'];
  senderEmail: Scalars['String']['output'];
  smtpHost: Scalars['String']['output'];
  smtpPort: Scalars['Int']['output'];
  tlsEnabled: Scalars['Boolean']['output'];
  username: Scalars['String']['output'];
};

/** The configuration for an [EmailerClient] */
export type EmailerClientConfig = {
  /** The SMTP host to use */
  host: Scalars['String']['input'];
  /** The maximum size of an attachment in bytes */
  maxAttachmentSizeBytes?: InputMaybe<Scalars['Int']['input']>;
  /** The maximum number of attachments that can be sent in a single email */
  maxNumAttachments?: InputMaybe<Scalars['Int']['input']>;
  /**
   * The plaintext password to use for the SMTP server, which will be encrypted before being stored.
   * This field is optional to support reusing the config for emailer config updates. If the password is not
   * set, it will error when trying to send an email.
   */
  password?: InputMaybe<Scalars['String']['input']>;
  /** The SMTP port to use */
  port: Scalars['Int']['input'];
  /** The display name to use for the sender */
  senderDisplayName: Scalars['String']['input'];
  /** The email address to send from */
  senderEmail: Scalars['String']['input'];
  /** Whether to use TLS for the SMTP connection */
  tlsEnabled: Scalars['Boolean']['input'];
  /** The username to use for the SMTP server, typically the same as the sender email */
  username: Scalars['String']['input'];
};

/** Input object for creating or updating an emailer */
export type EmailerInput = {
  /** The emailer configuration */
  config: EmailerClientConfig;
  /** Whether the emailer is the primary emailer */
  isPrimary: Scalars['Boolean']['input'];
  /** The friendly name of the emailer, e.g. "Aaron's Kobo" */
  name: Scalars['String']['input'];
};

export type EmailerSendRecord = {
  __typename?: 'EmailerSendRecord';
  attachmentMeta: Array<AttachmentMeta>;
  emailerId: Scalars['Int']['output'];
  id: Scalars['Int']['output'];
  recipientEmail: Scalars['String']['output'];
  sentAt: Scalars['DateTime']['output'];
  sentBy?: Maybe<User>;
  sentByUserId?: Maybe<Scalars['String']['output']>;
};

export type EmailerSendTo =
  { anonymous: SendToEmail; device?: never; }
  |  { anonymous?: never; device: SendToDevice; };

/** The visibility of a shareable entity */
export enum EntityVisibility {
  Private = 'PRIVATE',
  Public = 'PUBLIC',
  Shared = 'SHARED'
}

export type Epub = {
  __typename?: 'Epub';
  annotations: Array<MediaAnnotationModel>;
  bookmarks: Array<Bookmark>;
  extraCss: Array<Scalars['String']['output']>;
  media: Media;
  mediaId: Scalars['String']['output'];
  metadata: Scalars['JSONObject']['output'];
  resources: Scalars['JSONObject']['output'];
  rootBase: Scalars['String']['output'];
  rootFile: Scalars['String']['output'];
  spine: Array<SpineItem>;
  toc: Array<Scalars['String']['output']>;
};

export type EpubProgressInput = {
  deviceId?: InputMaybe<Scalars['String']['input']>;
  elapsedSecondsDelta?: InputMaybe<Scalars['Int']['input']>;
  isComplete?: InputMaybe<Scalars['Boolean']['input']>;
  locator: ReadiumLocatorInput;
  percentage?: InputMaybe<Scalars['Decimal']['input']>;
  resetElapsedSeconds?: InputMaybe<Scalars['Boolean']['input']>;
};

/**
 * A resize option which will resize the image to the given dimensions, without
 * maintaining the aspect ratio.
 */
export type ExactDimensionResize = {
  __typename?: 'ExactDimensionResize';
  /** The height (in pixels) the resulting image should be resized to */
  height: Scalars['Int']['output'];
  /** The width (in pixels) the resulting image should be resized to */
  width: Scalars['Int']['output'];
};

/**
 * A resize option which will resize the image to the given dimensions, without
 * maintaining the aspect ratio.
 */
export type ExactDimensionResizeInput = {
  /** The height (in pixels) the resulting image should be resized to */
  height: Scalars['Int']['input'];
  /** The width (in pixels) the resulting image should be resized to */
  width: Scalars['Int']['input'];
};

/** Metadata about a media item from an external metadata provider */
export type ExternalMediaMetadata = {
  __typename?: 'ExternalMediaMetadata';
  artists?: Maybe<Array<Scalars['String']['output']>>;
  colorists?: Maybe<Array<Scalars['String']['output']>>;
  coverArtists?: Maybe<Array<Scalars['String']['output']>>;
  coverUrl?: Maybe<Scalars['String']['output']>;
  day?: Maybe<Scalars['Int']['output']>;
  externalId: Scalars['String']['output'];
  genres?: Maybe<Array<Scalars['String']['output']>>;
  isbn?: Maybe<Scalars['String']['output']>;
  isbn13?: Maybe<Scalars['String']['output']>;
  letterers?: Maybe<Array<Scalars['String']['output']>>;
  month?: Maybe<Scalars['Int']['output']>;
  number?: Maybe<Scalars['Float']['output']>;
  pageCount?: Maybe<Scalars['Int']['output']>;
  provider: Scalars['String']['output'];
  providerUrl?: Maybe<Scalars['String']['output']>;
  seriesExternalId?: Maybe<Scalars['String']['output']>;
  seriesName?: Maybe<Scalars['String']['output']>;
  summary?: Maybe<Scalars['String']['output']>;
  tags?: Maybe<Array<Scalars['String']['output']>>;
  title?: Maybe<Scalars['String']['output']>;
  writers?: Maybe<Array<Scalars['String']['output']>>;
  year?: Maybe<Scalars['Int']['output']>;
};

export type ExternalMetadata = ExternalMediaMetadata | ExternalSeriesMetadata;

/** Metadata about a series from an external metadata provider */
export type ExternalSeriesMetadata = {
  __typename?: 'ExternalSeriesMetadata';
  ageRating?: Maybe<Scalars['String']['output']>;
  alternativeTitles: Array<Scalars['String']['output']>;
  artists?: Maybe<Array<Scalars['String']['output']>>;
  authors?: Maybe<Array<Scalars['String']['output']>>;
  coverUrl?: Maybe<Scalars['String']['output']>;
  endYear?: Maybe<Scalars['Int']['output']>;
  externalId: Scalars['String']['output'];
  genres?: Maybe<Array<Scalars['String']['output']>>;
  provider: Scalars['String']['output'];
  publisher?: Maybe<Scalars['String']['output']>;
  status?: Maybe<PublicationStatus>;
  summary?: Maybe<Scalars['String']['output']>;
  tags?: Maybe<Array<Scalars['String']['output']>>;
  title: Scalars['String']['output'];
  volumeCount?: Maybe<Scalars['Int']['output']>;
  year?: Maybe<Scalars['Int']['output']>;
};

export type FieldFilterFileStatus =
  { anyOf: Array<FileStatus>; contains?: never; endsWith?: never; eq?: never; excludes?: never; hasAnyOf?: never; hasNoneOf?: never; like?: never; likeAnyOf?: never; likeNoneOf?: never; neq?: never; noneOf?: never; startsWith?: never; }
  |  { anyOf?: never; contains: FileStatus; endsWith?: never; eq?: never; excludes?: never; hasAnyOf?: never; hasNoneOf?: never; like?: never; likeAnyOf?: never; likeNoneOf?: never; neq?: never; noneOf?: never; startsWith?: never; }
  |  { anyOf?: never; contains?: never; endsWith: FileStatus; eq?: never; excludes?: never; hasAnyOf?: never; hasNoneOf?: never; like?: never; likeAnyOf?: never; likeNoneOf?: never; neq?: never; noneOf?: never; startsWith?: never; }
  |  { anyOf?: never; contains?: never; endsWith?: never; eq: FileStatus; excludes?: never; hasAnyOf?: never; hasNoneOf?: never; like?: never; likeAnyOf?: never; likeNoneOf?: never; neq?: never; noneOf?: never; startsWith?: never; }
  |  { anyOf?: never; contains?: never; endsWith?: never; eq?: never; excludes: FileStatus; hasAnyOf?: never; hasNoneOf?: never; like?: never; likeAnyOf?: never; likeNoneOf?: never; neq?: never; noneOf?: never; startsWith?: never; }
  |  /**
   * Matches when a comma-separated list column (e.g. genres, writers) contains at
   * least one of the given values as a whole element. Unlike `likeAnyOf`, the value
   * `Science` does not match `Science fiction`.
   */
  { anyOf?: never; contains?: never; endsWith?: never; eq?: never; excludes?: never; hasAnyOf: Array<FileStatus>; hasNoneOf?: never; like?: never; likeAnyOf?: never; likeNoneOf?: never; neq?: never; noneOf?: never; startsWith?: never; }
  |  /** Matches when none of the given values is a whole element of the list column */
  { anyOf?: never; contains?: never; endsWith?: never; eq?: never; excludes?: never; hasAnyOf?: never; hasNoneOf: Array<FileStatus>; like?: never; likeAnyOf?: never; likeNoneOf?: never; neq?: never; noneOf?: never; startsWith?: never; }
  |  { anyOf?: never; contains?: never; endsWith?: never; eq?: never; excludes?: never; hasAnyOf?: never; hasNoneOf?: never; like: FileStatus; likeAnyOf?: never; likeNoneOf?: never; neq?: never; noneOf?: never; startsWith?: never; }
  |  { anyOf?: never; contains?: never; endsWith?: never; eq?: never; excludes?: never; hasAnyOf?: never; hasNoneOf?: never; like?: never; likeAnyOf: Array<FileStatus>; likeNoneOf?: never; neq?: never; noneOf?: never; startsWith?: never; }
  |  { anyOf?: never; contains?: never; endsWith?: never; eq?: never; excludes?: never; hasAnyOf?: never; hasNoneOf?: never; like?: never; likeAnyOf?: never; likeNoneOf: Array<FileStatus>; neq?: never; noneOf?: never; startsWith?: never; }
  |  { anyOf?: never; contains?: never; endsWith?: never; eq?: never; excludes?: never; hasAnyOf?: never; hasNoneOf?: never; like?: never; likeAnyOf?: never; likeNoneOf?: never; neq: FileStatus; noneOf?: never; startsWith?: never; }
  |  { anyOf?: never; contains?: never; endsWith?: never; eq?: never; excludes?: never; hasAnyOf?: never; hasNoneOf?: never; like?: never; likeAnyOf?: never; likeNoneOf?: never; neq?: never; noneOf: Array<FileStatus>; startsWith?: never; }
  |  { anyOf?: never; contains?: never; endsWith?: never; eq?: never; excludes?: never; hasAnyOf?: never; hasNoneOf?: never; like?: never; likeAnyOf?: never; likeNoneOf?: never; neq?: never; noneOf?: never; startsWith: FileStatus; };

export type FieldFilterString =
  { anyOf: Array<Scalars['String']['input']>; contains?: never; endsWith?: never; eq?: never; excludes?: never; hasAnyOf?: never; hasNoneOf?: never; like?: never; likeAnyOf?: never; likeNoneOf?: never; neq?: never; noneOf?: never; startsWith?: never; }
  |  { anyOf?: never; contains: Scalars['String']['input']; endsWith?: never; eq?: never; excludes?: never; hasAnyOf?: never; hasNoneOf?: never; like?: never; likeAnyOf?: never; likeNoneOf?: never; neq?: never; noneOf?: never; startsWith?: never; }
  |  { anyOf?: never; contains?: never; endsWith: Scalars['String']['input']; eq?: never; excludes?: never; hasAnyOf?: never; hasNoneOf?: never; like?: never; likeAnyOf?: never; likeNoneOf?: never; neq?: never; noneOf?: never; startsWith?: never; }
  |  { anyOf?: never; contains?: never; endsWith?: never; eq: Scalars['String']['input']; excludes?: never; hasAnyOf?: never; hasNoneOf?: never; like?: never; likeAnyOf?: never; likeNoneOf?: never; neq?: never; noneOf?: never; startsWith?: never; }
  |  { anyOf?: never; contains?: never; endsWith?: never; eq?: never; excludes: Scalars['String']['input']; hasAnyOf?: never; hasNoneOf?: never; like?: never; likeAnyOf?: never; likeNoneOf?: never; neq?: never; noneOf?: never; startsWith?: never; }
  |  /**
   * Matches when a comma-separated list column (e.g. genres, writers) contains at
   * least one of the given values as a whole element. Unlike `likeAnyOf`, the value
   * `Science` does not match `Science fiction`.
   */
  { anyOf?: never; contains?: never; endsWith?: never; eq?: never; excludes?: never; hasAnyOf: Array<Scalars['String']['input']>; hasNoneOf?: never; like?: never; likeAnyOf?: never; likeNoneOf?: never; neq?: never; noneOf?: never; startsWith?: never; }
  |  /** Matches when none of the given values is a whole element of the list column */
  { anyOf?: never; contains?: never; endsWith?: never; eq?: never; excludes?: never; hasAnyOf?: never; hasNoneOf: Array<Scalars['String']['input']>; like?: never; likeAnyOf?: never; likeNoneOf?: never; neq?: never; noneOf?: never; startsWith?: never; }
  |  { anyOf?: never; contains?: never; endsWith?: never; eq?: never; excludes?: never; hasAnyOf?: never; hasNoneOf?: never; like: Scalars['String']['input']; likeAnyOf?: never; likeNoneOf?: never; neq?: never; noneOf?: never; startsWith?: never; }
  |  { anyOf?: never; contains?: never; endsWith?: never; eq?: never; excludes?: never; hasAnyOf?: never; hasNoneOf?: never; like?: never; likeAnyOf: Array<Scalars['String']['input']>; likeNoneOf?: never; neq?: never; noneOf?: never; startsWith?: never; }
  |  { anyOf?: never; contains?: never; endsWith?: never; eq?: never; excludes?: never; hasAnyOf?: never; hasNoneOf?: never; like?: never; likeAnyOf?: never; likeNoneOf: Array<Scalars['String']['input']>; neq?: never; noneOf?: never; startsWith?: never; }
  |  { anyOf?: never; contains?: never; endsWith?: never; eq?: never; excludes?: never; hasAnyOf?: never; hasNoneOf?: never; like?: never; likeAnyOf?: never; likeNoneOf?: never; neq: Scalars['String']['input']; noneOf?: never; startsWith?: never; }
  |  { anyOf?: never; contains?: never; endsWith?: never; eq?: never; excludes?: never; hasAnyOf?: never; hasNoneOf?: never; like?: never; likeAnyOf?: never; likeNoneOf?: never; neq?: never; noneOf: Array<Scalars['String']['input']>; startsWith?: never; }
  |  { anyOf?: never; contains?: never; endsWith?: never; eq?: never; excludes?: never; hasAnyOf?: never; hasNoneOf?: never; like?: never; likeAnyOf?: never; likeNoneOf?: never; neq?: never; noneOf?: never; startsWith: Scalars['String']['input']; };

/** The different statuses a file reference can have */
export enum FileStatus {
  Error = 'ERROR',
  Missing = 'MISSING',
  Ready = 'READY',
  Unknown = 'UNKNOWN',
  Unsupported = 'UNSUPPORTED'
}

export enum FilterableArrangementEntity {
  Books = 'BOOKS',
  BookClubs = 'BOOK_CLUBS',
  Libraries = 'LIBRARIES',
  Series = 'SERIES',
  SmartLists = 'SMART_LISTS'
}

export enum FilterableArrangementEntityLink {
  Create = 'CREATE',
  ShowAll = 'SHOW_ALL'
}

export type FilterableArrangementEntityLinkInput = {
  entity: FilterableArrangementEntity;
  filter?: InputMaybe<Scalars['JSON']['input']>;
  links?: Array<FilterableArrangementEntityLink>;
  name?: InputMaybe<Scalars['String']['input']>;
  orderBy?: InputMaybe<Scalars['String']['input']>;
};

/**
 * A resize option which will resize the image to fit within the given dimensions,
 * maintaining the aspect ratio.
 *
 * If the image already fits within the dimensions, it will not be scaled up.
 */
export type FitWithinResize = {
  __typename?: 'FitWithinResize';
  /** The maximum height (in pixels) of the resulting image */
  height: Scalars['Int']['output'];
  /** The maximum width (in pixels) of the resulting image */
  width: Scalars['Int']['output'];
};

/**
 * A resize option which will resize the image to fit within the given dimensions,
 * maintaining the aspect ratio.
 *
 * If the image already fits within the dimensions, it will not be scaled up.
 */
export type FitWithinResizeInput = {
  /** The maximum height (in pixels) of the resulting image */
  height: Scalars['Int']['input'];
  /** The maximum width (in pixels) of the resulting image */
  width: Scalars['Int']['input'];
};

/**
 * A node in the global reading timeline, which is more of a flat list of sessions with their
 * corresponding events instead of being grouped by readthroughs
 */
export type GlobalReadingTimelineNode = {
  __typename?: 'GlobalReadingTimelineNode';
  mediaId: Scalars['String']['output'];
  session: SessionWithEvents;
};

/**
 * A file Stump took out of a library and holds in its trash folder for the retention: the
 * original of a book post-processing replaced, or the version a put-back replaced
 */
export type HeldFile = {
  __typename?: 'HeldFile';
  /** The book, when it is still in Stump (removed books included) */
  book?: Maybe<Media>;
  bytes: Scalars['Int']['output'];
  /** Whether it can be put back: its file is in the trash folder and its book is in Stump */
  canPutBack: Scalars['Boolean']['output'];
  /** When its time in Deleted books is over (null: kept until deleted by hand) */
  expiresAt?: Maybe<Scalars['DateTime']['output']>;
  heldAt: Scalars['DateTime']['output'];
  /** Who made Stump take it out (null: post-processing) */
  heldBy?: Maybe<Scalars['String']['output']>;
  id: Scalars['Int']['output'];
  libraryId?: Maybe<Scalars['String']['output']>;
  libraryName?: Maybe<Scalars['String']['output']>;
  /** When the automatic purge will delete it */
  purgeAt?: Maybe<Scalars['DateTime']['output']>;
  /** Why it was taken out */
  reason: HeldFileReason;
  /** Where it was, inside its library */
  relativePath: Scalars['String']['output'];
  /** For a PDF: the post-processing steps asked for (`OCR`, `SHRINK`, `OCR+SHRINK`) */
  steps?: Maybe<Scalars['String']['output']>;
  /** The book's title when the file was held */
  title: Scalars['String']['output'];
};

export type HeldFileKept = {
  __typename?: 'HeldFileKept';
  error: Scalars['String']['output'];
  id: Scalars['Int']['output'];
  title: Scalars['String']['output'];
};

/** What putting a held file back did */
export type HeldFilePutBack = {
  __typename?: 'HeldFilePutBack';
  /** The book, with its file back */
  book: Media;
  /** The version that was in the library, now held in turn (null: there was none) */
  replaced?: Maybe<Scalars['Int']['output']>;
};

/** Why Stump holds a file in its trash folder */
export enum HeldFileReason {
  /** The original of a manga volume reduced to 16 grey levels */
  Grey_16 = 'GREY_16',
  /** The original of a PDF the PDF worker OCRed and/or shrank */
  Pdf = 'PDF',
  /** The version of a book that was in the library when its held original was put back */
  PutBack = 'PUT_BACK'
}

/** What deleting held files did */
export type HeldFilesDeleteResult = {
  __typename?: 'HeldFilesDeleteResult';
  deleted: Scalars['Int']['output'];
  freedBytes: Scalars['Int']['output'];
  /** The ones kept, with why */
  kept: Array<HeldFileKept>;
};

/** The sections displayed on a user's home page. */
export type HomeArrangement = {
  __typename?: 'HomeArrangement';
  sections: Array<ArrangementSection>;
};

export type HomeArrangementInput = {
  sections: Array<ArrangementSectionInput>;
};

export type ImageColor = {
  __typename?: 'ImageColor';
  color: Scalars['String']['output'];
  percentage: Scalars['Decimal']['output'];
};

export type ImageDimensions = {
  __typename?: 'ImageDimensions';
  height: Scalars['Int']['output'];
  width: Scalars['Int']['output'];
};

export type ImageMetadata = {
  __typename?: 'ImageMetadata';
  averageColor?: Maybe<Scalars['String']['output']>;
  colors: Array<ImageColor>;
  dimensions?: Maybe<ImageDimensions>;
  thumbhash?: Maybe<Scalars['String']['output']>;
};

/** Options for processing images throughout Stump. */
export type ImageProcessorOptions = {
  __typename?: 'ImageProcessorOptions';
  /** The format to use when generating an image. See [`SupportedImageFormat`] */
  format: SupportedImageFormat;
  /** The page to use when generating an image. This is not applicable to all media formats. */
  page?: Maybe<Scalars['Int']['output']>;
  /**
   * The quality to use when generating an image. This is a number between 1 and 100,
   * where 100 is the highest quality. Omitting this value will use the default quality
   * of 100.
   */
  quality?: Maybe<Scalars['Int']['output']>;
  /** The size factor to use when generating an image. See [`ImageResizeOptions`] */
  resizeMethod?: Maybe<ImageResizeMethod>;
};

/** Options for processing images throughout Stump. */
export type ImageProcessorOptionsInput = {
  /** The format to use when generating an image. See [`SupportedImageFormat`] */
  format: SupportedImageFormat;
  /** The page to use when generating an image. This is not applicable to all media formats. */
  page?: InputMaybe<Scalars['Int']['input']>;
  /**
   * The quality to use when generating an image. This is a number between 1 and 100,
   * where 100 is the highest quality. Omitting this value will use the default quality
   * of 100.
   */
  quality?: InputMaybe<Scalars['Int']['input']>;
  /** The size factor to use when generating an image. See [`ImageResizeOptions`] */
  resizeMethod?: InputMaybe<ImageResizeMethodInput>;
};

export type ImageRef = {
  __typename?: 'ImageRef';
  height?: Maybe<Scalars['Int']['output']>;
  lastModified?: Maybe<Scalars['DateTime']['output']>;
  metadata?: Maybe<ImageMetadata>;
  url: Scalars['String']['output'];
  width?: Maybe<Scalars['Int']['output']>;
};

/** The resize options to use when generating an image */
export type ImageResizeMethod = ExactDimensionResize | FitWithinResize | ScaleEvenlyByFactor | ScaledDimensionResize;

/** The resize options to use when generating an image */
export type ImageResizeMethodInput =
  { exact: ExactDimensionResizeInput; fitWithin?: never; scaleDimension?: never; scaleEvenlyByFactor?: never; }
  |  { exact?: never; fitWithin: FitWithinResizeInput; scaleDimension?: never; scaleEvenlyByFactor?: never; }
  |  { exact?: never; fitWithin?: never; scaleDimension: ScaledDimensionResizeInput; scaleEvenlyByFactor?: never; }
  |  { exact?: never; fitWithin?: never; scaleDimension?: never; scaleEvenlyByFactor: ScaleEvenlyByFactorInput; };

export type InProgressBooks = {
  __typename?: 'InProgressBooks';
  name?: Maybe<Scalars['String']['output']>;
};

export type InProgressBooksInput = {
  name?: InputMaybe<Scalars['String']['input']>;
};

export type InheritPermissionStruct = {
  __typename?: 'InheritPermissionStruct';
  value: InheritPermissionValue;
};

export enum InheritPermissionValue {
  Inherit = 'INHERIT'
}

/** The values that will be filed: the proposal's defaults, then the uploader's edits */
export type IntakeDecision = {
  __typename?: 'IntakeDecision';
  /** The file name per uploaded file (same order as the item's files), without folder */
  fileNames: Array<Scalars['String']['output']>;
  /** The author folder (books) or the series folder (series), a single path component */
  folder: Scalars['String']['output'];
  metadata: IntakeMetadata;
  /**
   * `None` files a book under its author folder; `Some` files it (or every volume)
   * under the manga or light novel root
   */
  seriesKind?: Maybe<IntakeSeriesKind>;
  /** Per-file volume numbers for series (same order as the item's files) */
  volumes: Array<Maybe<Scalars['Float']['output']>>;
};

export type IntakeDestination = {
  __typename?: 'IntakeDestination';
  /** Whether that folder already exists */
  dirExists: Scalars['Boolean']['output'];
  files: Array<IntakeDestinationFile>;
  /** The folder, relative to the library root */
  relativeDir: Scalars['String']['output'];
};

/** Where each file of an item will go (or went) */
export type IntakeDestinationFile = {
  __typename?: 'IntakeDestinationFile';
  /** Something already exists at that path (filing will fail for this file) */
  exists: Scalars['Boolean']['output'];
  originalName: Scalars['String']['output'];
  /** Relative to the library root, e.g. `Umberto_Eco/El_nombre_de_la_rosa.epub` */
  relativePath: Scalars['String']['output'];
};

/** A book in the library that looks like the same book */
export type IntakeDuplicate = {
  __typename?: 'IntakeDuplicate';
  /** The uploaded file it matches */
  fileName: Scalars['String']['output'];
  mediaId: Scalars['String']['output'];
  path: Scalars['String']['output'];
  /** `SAME_FILE` (identical hash) or `SAME_TITLE` (same author and title) */
  reason: Scalars['String']['output'];
  title: Scalars['String']['output'];
};

export type IntakeFieldSource = {
  __typename?: 'IntakeFieldSource';
  /**
   * The camelCase name of the field (`title`, `authors`, `series`, `seriesIndex`,
   * `language`, `year`, `publisher`, `description`, `isbn`, `subjects`)
   */
  field: Scalars['String']['output'];
  source: IntakeSource;
};

/** One uploaded file of an item (a book, or a volume of a series) */
export type IntakeFile = {
  __typename?: 'IntakeFile';
  /** `epub`, `pdf`, `cbz`, `cbr` */
  format: Scalars['String']['output'];
  /** Stump's own file hash, used to find duplicates */
  hash?: Maybe<Scalars['String']['output']>;
  /** The name it was uploaded with (for a folder upload, its path inside the folder) */
  originalName: Scalars['String']['output'];
  size: Scalars['Int']['output'];
  /** The title read from this file (volumes may carry their own) */
  title?: Maybe<Scalars['String']['output']>;
  /** The volume number, for a series */
  volume?: Maybe<Scalars['Float']['output']>;
};

/** An existing author or series folder, for the folder picker */
export type IntakeFolder = {
  __typename?: 'IntakeFolder';
  books: Scalars['Int']['output'];
  name: Scalars['String']['output'];
};

/** An existing (or proposed new) folder the item can go to */
export type IntakeFolderCandidate = {
  __typename?: 'IntakeFolderCandidate';
  /** How many books the library already has in it */
  books: Scalars['Int']['output'];
  /** 0-1 */
  confidence: Scalars['Float']['output'];
  /**
   * The folder name, relative to the library root (author folders) or to the series
   * root (series folders)
   */
  folder: Scalars['String']['output'];
  /** The folder does not exist yet and would be created */
  isNew: Scalars['Boolean']['output'];
  /** Why it is suggested, e.g. "same author written 'Eco, Umberto'" */
  reason: Scalars['String']['output'];
};

/** A tag proposed for the item, taken from the tags the library already uses */
export type IntakeGenreSuggestion = {
  __typename?: 'IntakeGenreSuggestion';
  /** 0-1 */
  confidence: Scalars['Float']['output'];
  /** `GENRE` (a `Big` or `Big: Small` tag) or `FACET` (`Origin:`, `Era:`, `Level:`...) */
  kind: Scalars['String']['output'];
  /** How many books of the library carry it */
  libraryCount: Scalars['Int']['output'];
  /** Short explanations, e.g. "3 other books by this author have it" */
  reasons: Array<Scalars['String']['output']>;
  /** Whether it is part of the default choice */
  selected: Scalars['Boolean']['output'];
  value: Scalars['String']['output'];
};

/** An item of the uploader's "Add books" inbox */
export type IntakeItem = {
  __typename?: 'IntakeItem';
  createdAt: Scalars['DateTime']['output'];
  /** What will be filed: the proposal's defaults plus the uploader's edits */
  decision?: Maybe<IntakeDecision>;
  /** Where each file goes, relative to the library root */
  destination?: Maybe<IntakeDestination>;
  error?: Maybe<Scalars['String']['output']>;
  filedAt?: Maybe<Scalars['DateTime']['output']>;
  files: Array<IntakeFile>;
  id: Scalars['ID']['output'];
  kind: IntakeKind;
  libraryId: Scalars['String']['output'];
  libraryName: Scalars['String']['output'];
  /** The books created by the scan after filing (empty until the scan has run) */
  mediaIds: Array<Scalars['String']['output']>;
  /** The uploaded file or folder name */
  name: Scalars['String']['output'];
  /** What still keeps the decision from being filed (empty when it can be accepted) */
  problems: Array<Scalars['String']['output']>;
  /** What the server proposes (empty while analysing) */
  proposal?: Maybe<IntakeProposal>;
  /** What filing did */
  result?: Maybe<IntakeResult>;
  state: IntakeState;
  updatedAt: Scalars['DateTime']['output'];
};

/** A single book, or a series (a folder or zip of volumes: manga, light novels) */
export enum IntakeKind {
  Book = 'BOOK',
  Series = 'SERIES'
}

/** The descriptive metadata of an item, with where each value came from */
export type IntakeMetadata = {
  __typename?: 'IntakeMetadata';
  /** 0 (all ages) or 18 (adults only), when known */
  ageRating?: Maybe<Scalars['Int']['output']>;
  authors: Array<Scalars['String']['output']>;
  description?: Maybe<Scalars['String']['output']>;
  /**
   * The tags that will be written: library genres (`Big`, `Big: Small`) and facets
   * (`Origin: X`, `Era: X`, ...)
   */
  genres: Array<Scalars['String']['output']>;
  isbn?: Maybe<Scalars['String']['output']>;
  /** Two-letter code (`en`, `es`, ...) */
  language?: Maybe<Scalars['String']['output']>;
  publisher?: Maybe<Scalars['String']['output']>;
  /** 0-5 */
  rating?: Maybe<Scalars['Float']['output']>;
  series?: Maybe<Scalars['String']['output']>;
  seriesIndex?: Maybe<Scalars['Float']['output']>;
  sources: Array<IntakeFieldSource>;
  /** Raw subjects found in the file and online, before matching to the library's tags */
  subjects: Array<Scalars['String']['output']>;
  title?: Maybe<Scalars['String']['output']>;
  /** First publication year when known (Open Library), else the edition year */
  year?: Maybe<Scalars['Int']['output']>;
};

/** What the server proposes after analysing an item */
export type IntakeProposal = {
  __typename?: 'IntakeProposal';
  /** Author folders, best first (books) */
  authorFolders: Array<IntakeFolderCandidate>;
  /** Overall confidence, 0-1; under the threshold the item needs review */
  confidence: Scalars['Float']['output'];
  duplicates: Array<IntakeDuplicate>;
  /** Tags from the library's vocabulary, best first */
  genreSuggestions: Array<IntakeGenreSuggestion>;
  /** Whether an online source found the book (a failed or empty lookup isn't one) */
  lookedUpOnline: Scalars['Boolean']['output'];
  /** The merged metadata (file, file name, online lookups), with sources */
  metadata: IntakeMetadata;
  /** How each online source answered, when they were asked */
  onlineLookups: Array<OnlineLookup>;
  /** Series folders, best first (series and series volumes) */
  seriesFolders: Array<IntakeFolderCandidate>;
  /** Set when the item looks like manga or a light novel */
  seriesKind?: Maybe<IntakeSeriesKind>;
  similarBooks: Array<IntakeSimilarBook>;
  /** Plain-language notes for the uploader (e.g. "No author found") */
  warnings: Array<Scalars['String']['output']>;
};

/** What filing did */
export type IntakeResult = {
  __typename?: 'IntakeResult';
  /** Notes, e.g. a metadata field that could not be written into a PDF */
  notes: Array<Scalars['String']['output']>;
  /** Absolute paths of the filed files */
  paths: Array<Scalars['String']['output']>;
};

/** The uploader's review of an item. Omitted fields keep their current value. */
export type IntakeReviewInput = {
  ageRating?: InputMaybe<Scalars['Int']['input']>;
  authors?: InputMaybe<Array<Scalars['String']['input']>>;
  clearSeriesKind?: Scalars['Boolean']['input'];
  description?: InputMaybe<Scalars['String']['input']>;
  /** One per file, same order as the item's files */
  fileNames?: InputMaybe<Array<Scalars['String']['input']>>;
  /** The author or series folder (a single path component; may be new) */
  folder?: InputMaybe<Scalars['String']['input']>;
  genres?: InputMaybe<Array<Scalars['String']['input']>>;
  language?: InputMaybe<Scalars['String']['input']>;
  publisher?: InputMaybe<Scalars['String']['input']>;
  series?: InputMaybe<Scalars['String']['input']>;
  seriesIndex?: InputMaybe<Scalars['Float']['input']>;
  /**
   * File the item as a series under this root. Use `clearSeriesKind` to file a book
   * under its author folder again
   */
  seriesKind?: InputMaybe<IntakeSeriesKind>;
  title?: InputMaybe<Scalars['String']['input']>;
  /** One per file, same order as the item's files */
  volumes?: InputMaybe<Array<InputMaybe<Scalars['Float']['input']>>>;
  year?: InputMaybe<Scalars['Int']['input']>;
};

/** Which series root a series item (or a single volume) goes to */
export enum IntakeSeriesKind {
  LightNovel = 'LIGHT_NOVEL',
  Manga = 'MANGA'
}

/** A book already in the library, shown to compare against */
export type IntakeSimilarBook = {
  __typename?: 'IntakeSimilarBook';
  authors: Array<Scalars['String']['output']>;
  genres: Array<Scalars['String']['output']>;
  mediaId: Scalars['String']['output'];
  /** Why it is similar (`same author`, `same series`, `similar description`) */
  reason: Scalars['String']['output'];
  title: Scalars['String']['output'];
};

/** Where a metadata value came from, so the review screen can say so */
export enum IntakeSource {
  /** The book file's own metadata (OPF, PDF Info, ComicInfo) */
  File = 'FILE',
  /** Parsed from the file or folder name */
  FileName = 'FILE_NAME',
  GoogleBooks = 'GOOGLE_BOOKS',
  Hardcover = 'HARDCOVER',
  /** Inferred from books already in the library (same author, series, similar text) */
  Library = 'LIBRARY',
  OpenLibrary = 'OPEN_LIBRARY',
  /** Set by the uploader */
  User = 'USER'
}

/** Where an inbox item is in its life */
export enum IntakeState {
  /** Uploaded; its metadata is being read, looked up and matched against the library */
  Analysing = 'ANALYSING',
  /** Thrown away by the uploader; its staged files are deleted */
  Discarded = 'DISCARDED',
  /** Analysis or filing failed, see `error` */
  Failed = 'FAILED',
  /** Written into the library */
  Filed = 'FILED',
  /** Something important is missing or uncertain (author folder, genres, title...) */
  NeedsReview = 'NEEDS_REVIEW',
  /** Everything needed to file it is known and confident; it can be accepted as is */
  Ready = 'READY'
}

/** How an upload of several files is grouped into inbox items */
export enum IntakeUploadMode {
  /**
   * Files in a folder (or books inside a .zip) make one series item; loose files are
   * one book each
   */
  Auto = 'AUTO',
  /** Every file is its own book */
  Books = 'BOOKS',
  /** All the files are the volumes of one series */
  Series = 'SERIES'
}

/** A tag the library uses, for the tag picker */
export type IntakeVocabularyEntry = {
  __typename?: 'IntakeVocabularyEntry';
  /** How many books carry it */
  count: Scalars['Int']['output'];
  /** `GENRE` or `FACET` */
  kind: Scalars['String']['output'];
  value: Scalars['String']['output'];
};

/**
 * The different types of layouts a client-side interface might present to a user
 * for a collection of items
 */
export enum InterfaceLayout {
  Grid = 'GRID',
  Table = 'TABLE'
}

/** The roundness of certain UI elements in the client interface, such as cards, buttons, inputs, etc */
export enum InterfaceRoundness {
  None = 'NONE',
  Normal = 'NORMAL',
  Pill = 'PILL',
  Rounded = 'ROUNDED'
}

export type Job = {
  __typename?: 'Job';
  completedAt?: Maybe<Scalars['DateTime']['output']>;
  createdAt: Scalars['DateTime']['output'];
  description?: Maybe<Scalars['String']['output']>;
  id: Scalars['String']['output'];
  logCount: Scalars['Int']['output'];
  logs: Array<Log>;
  msElapsed: Scalars['Int']['output'];
  name: Scalars['String']['output'];
  outputData?: Maybe<CoreJobOutput>;
  saveState?: Maybe<Scalars['JSON']['output']>;
  status: JobStatus;
};

export type JobOutput = {
  __typename?: 'JobOutput';
  id: Scalars['String']['output'];
  output: CoreJobOutput;
};

export type JobStarted = {
  __typename?: 'JobStarted';
  id: Scalars['String']['output'];
};

export enum JobStatus {
  Cancelled = 'CANCELLED',
  Completed = 'COMPLETED',
  Failed = 'FAILED',
  Paused = 'PAUSED',
  Queued = 'QUEUED',
  Running = 'RUNNING'
}

/** An update event that is emitted by a job */
export type JobUpdate = {
  __typename?: 'JobUpdate';
  /** The current subtask being worked on */
  completedSubtasks?: Maybe<Scalars['Int']['output']>;
  /** The current task being worked on */
  completedTasks?: Maybe<Scalars['Int']['output']>;
  id: Scalars['String']['output'];
  /** The primary message to display, e.g. "Scanning series" */
  message?: Maybe<Scalars['String']['output']>;
  /**
   * The number of tasks for the job. This number can change as
   * subtasks get added/converted to tasks
   */
  remainingTasks?: Maybe<Scalars['Int']['output']>;
  /** The status of the job */
  status?: Maybe<JobStatus>;
  /** An optional secondary message providing additional detail, e.g. a path or item name */
  subtitle?: Maybe<Scalars['String']['output']>;
  /** The number of subtasks that exist in the current task */
  totalSubtasks?: Maybe<Scalars['Int']['output']>;
};

export type JournalEntryModel = {
  __typename?: 'JournalEntryModel';
  content: Scalars['String']['output'];
  createdAt: Scalars['DateTime']['output'];
  id: Scalars['Int']['output'];
  mediaId: Scalars['String']['output'];
  sessionId?: Maybe<Scalars['Int']['output']>;
  updatedAt: Scalars['DateTime']['output'];
  userId: Scalars['String']['output'];
};

export type Library = {
  __typename?: 'Library';
  authors: Array<Author>;
  config: LibraryConfig;
  configId: Scalars['Int']['output'];
  createdAt: Scalars['DateTime']['output'];
  description?: Maybe<Scalars['String']['output']>;
  emoji?: Maybe<Scalars['String']['output']>;
  excludedUsers: Array<User>;
  genres: Array<Scalars['String']['output']>;
  id: Scalars['String']['output'];
  isFavorite: Scalars['Boolean']['output'];
  /** Get the details of the last scan job for this library, if any exists. */
  lastScan?: Maybe<LibraryScanRecord>;
  lastScannedAt?: Maybe<Scalars['DateTime']['output']>;
  /** Get media in this library */
  media: Array<Media>;
  mediaAlphabet: Scalars['JSONObject']['output'];
  name: Scalars['String']['output'];
  path: Scalars['String']['output'];
  publishers: Array<Scalars['String']['output']>;
  /** Get the full history of scan jobs for this library. */
  scanHistory: Array<LibraryScanRecord>;
  /** Get series in this library */
  series: Array<Series>;
  seriesAlphabet: Scalars['JSONObject']['output'];
  stats: LibraryStats;
  status: FileStatus;
  tags: Array<Tag>;
  /**
   * A reference to the thumbnail image for the thumbnail. This will be a fully
   * qualified URL to the image.
   */
  thumbnail: ImageRef;
  thumbnailMeta?: Maybe<ImageMetadata>;
  thumbnailPath?: Maybe<Scalars['String']['output']>;
  updatedAt?: Maybe<Scalars['DateTime']['output']>;
};


export type LibraryGenresArgs = {
  sort?: InputMaybe<OrderDirection>;
};


export type LibraryMediaArgs = {
  take?: InputMaybe<Scalars['Int']['input']>;
};


export type LibraryPublishersArgs = {
  sort?: InputMaybe<OrderDirection>;
};


export type LibrarySeriesArgs = {
  skip?: InputMaybe<Scalars['Int']['input']>;
  take?: InputMaybe<Scalars['Int']['input']>;
};


export type LibraryStatsArgs = {
  allUsers?: InputMaybe<Scalars['Boolean']['input']>;
};

export type LibraryConfig = {
  __typename?: 'LibraryConfig';
  convertRarToZip: Scalars['Boolean']['output'];
  defaultLibraryViewMode: LibraryViewMode;
  defaultReadingDir: ReadingDirection;
  defaultReadingImageScaleFit: ReadingImageScaleFit;
  defaultReadingMode: ReadingMode;
  generateFileHashes: Scalars['Boolean']['output'];
  generateKoreaderHashes: Scalars['Boolean']['output'];
  hardDeleteConversions: Scalars['Boolean']['output'];
  hideSeriesView: Scalars['Boolean']['output'];
  id: Scalars['Int']['output'];
  ignoreRules?: Maybe<Array<Scalars['String']['output']>>;
  libraryId?: Maybe<Scalars['String']['output']>;
  libraryPattern: LibraryPattern;
  libraryType: LibraryType;
  /**
   * the **relative path** to the directory where oneshots are stored,
   * relative to the library path. this is **not** a fully qualified path
   */
  oneshotsDirectory?: Maybe<Scalars['String']['output']>;
  organizationMode: OrganizationMode;
  processMetadata: Scalars['Boolean']['output'];
  processThumbnailColorsEvenWithoutConfig: Scalars['Boolean']['output'];
  skipBookOverview: Scalars['Boolean']['output'];
  thumbnailConfig?: Maybe<ImageProcessorOptions>;
  watch: Scalars['Boolean']['output'];
};

export type LibraryConfigInput = {
  convertRarToZip: Scalars['Boolean']['input'];
  defaultLibraryViewMode: LibraryViewMode;
  defaultReadingDir: ReadingDirection;
  defaultReadingImageScaleFit: ReadingImageScaleFit;
  defaultReadingMode: ReadingMode;
  generateFileHashes: Scalars['Boolean']['input'];
  generateKoreaderHashes: Scalars['Boolean']['input'];
  hardDeleteConversions: Scalars['Boolean']['input'];
  hideSeriesView: Scalars['Boolean']['input'];
  ignoreRules?: InputMaybe<Array<Scalars['String']['input']>>;
  libraryPattern: LibraryPattern;
  libraryType: LibraryType;
  oneshotsDirectory?: InputMaybe<Scalars['String']['input']>;
  organizationMode?: InputMaybe<OrganizationMode>;
  processMetadata: Scalars['Boolean']['input'];
  processThumbnailColorsEvenWithoutConfig: Scalars['Boolean']['input'];
  skipBookOverview: Scalars['Boolean']['input'];
  thumbnailConfig?: InputMaybe<ImageProcessorOptionsInput>;
  watch: Scalars['Boolean']['input'];
};

export type LibraryFilterInput = {
  _and?: InputMaybe<Array<LibraryFilterInput>>;
  _not?: InputMaybe<Array<LibraryFilterInput>>;
  _or?: InputMaybe<Array<LibraryFilterInput>>;
  id?: InputMaybe<FieldFilterString>;
  name?: InputMaybe<FieldFilterString>;
  path?: InputMaybe<FieldFilterString>;
};

/**
 * How the "Add books" intake files books into a library. A library without a row uses
 * [`Model::defaults_for`].
 */
export type LibraryIntakeSettings = {
  __typename?: 'LibraryIntakeSettings';
  libraryId: Scalars['String']['output'];
  /** Folder (relative to the library root) holding one folder per light novel series */
  lightNovelDir: Scalars['String']['output'];
  /** Folder (relative to the library root) holding one folder per manga series */
  mangaDir: Scalars['String']['output'];
  /** Look titles up on Open Library and Google Books while analysing */
  onlineLookup: Scalars['Boolean']['output'];
};

export type LibraryIntakeSettingsInput = {
  lightNovelDir: Scalars['String']['input'];
  mangaDir: Scalars['String']['input'];
  onlineLookup: Scalars['Boolean']['input'];
};

export type LibraryModelOrderBy = {
  direction: OrderDirection;
  field: LibraryModelOrdering;
};

export enum LibraryModelOrdering {
  ConfigId = 'CONFIG_ID',
  CreatedAt = 'CREATED_AT',
  Description = 'DESCRIPTION',
  Emoji = 'EMOJI',
  Id = 'ID',
  LastScannedAt = 'LAST_SCANNED_AT',
  Name = 'NAME',
  Path = 'PATH',
  Status = 'STATUS',
  ThumbnailMeta = 'THUMBNAIL_META',
  ThumbnailPath = 'THUMBNAIL_PATH',
  UpdatedAt = 'UPDATED_AT'
}

/** The different patterns a library may be organized by */
export enum LibraryPattern {
  CollectionBased = 'COLLECTION_BASED',
  SeriesBased = 'SERIES_BASED'
}

/** A library's post-processing settings and where its work stands */
export type LibraryPostProcessing = {
  __typename?: 'LibraryPostProcessing';
  /** Bytes saved by the replaced files */
  bytesSaved: Scalars['Int']['output'];
  config: PostProcessingConfig;
  /** Task counts by kind and status */
  counts: Array<PostProcessingCount>;
  /** The PDF queue shared with the PDF worker container, as Stump sees it */
  pdfQueueDir: Scalars['String']['output'];
  /** The PDF worker container, from its heartbeat in the queue folder */
  pdfWorker: PdfWorker;
  /**
   * Where replaced originals are kept for the *Deleted books* retention (Stump's trash
   * folder), as Stump sees it
   */
  trashDir: Scalars['String']['output'];
};

export type LibraryScanConfigInput = {
  /** Library IDs to scan. An empty list means "all libraries" */
  libraryIds: Array<Scalars['String']['input']>;
};

/** The data that is collected and updated during the execution of a library scan job */
export type LibraryScanOutput = {
  __typename?: 'LibraryScanOutput';
  /** The number of media entities created */
  createdMedia: Scalars['Int']['output'];
  /** The number of series entities created */
  createdSeries: Scalars['Int']['output'];
  /** The number of ignored directories during the scan */
  ignoredDirectories: Scalars['Int']['output'];
  /** The number of files that were ignored during the scan */
  ignoredFiles: Scalars['Int']['output'];
  /**
   * The number of files that were deemed to be skipped during the scan, e.g. it
   * exists in the database but has not been modified since the last scan
   */
  skippedFiles: Scalars['Int']['output'];
  /** The number of directories visited during the scan */
  totalDirectories: Scalars['Int']['output'];
  /** The number of files visited during the scan */
  totalFiles: Scalars['Int']['output'];
  /** The number of media entities updated */
  updatedMedia: Scalars['Int']['output'];
  /** The number of series entities updated */
  updatedSeries: Scalars['Int']['output'];
};

export type LibraryScanRecord = {
  __typename?: 'LibraryScanRecord';
  id: Scalars['Int']['output'];
  jobId?: Maybe<Scalars['String']['output']>;
  libraryId: Scalars['String']['output'];
  options?: Maybe<Scalars['JSON']['output']>;
  timestamp: Scalars['DateTime']['output'];
};

export type LibraryStats = {
  __typename?: 'LibraryStats';
  bookCount: Scalars['Int']['output'];
  completedBooks: Scalars['Int']['output'];
  inProgressBooks: Scalars['Int']['output'];
  seriesCount: Scalars['Int']['output'];
  totalBytes: Scalars['Int']['output'];
  totalReadingTimeSeconds: Scalars['Int']['output'];
};

/** The type of content a library contains */
export enum LibraryType {
  Book = 'BOOK',
  Comic = 'COMIC',
  LightNovel = 'LIGHT_NOVEL',
  Manga = 'MANGA',
  Manhwa = 'MANHWA',
  Mixed = 'MIXED',
  Webtoon = 'WEBTOON',
  WebNovel = 'WEB_NOVEL'
}

export enum LibraryViewMode {
  Books = 'BOOKS',
  Series = 'SERIES'
}

export type Log = {
  __typename?: 'Log';
  context?: Maybe<Scalars['String']['output']>;
  id: Scalars['Int']['output'];
  jobId?: Maybe<Scalars['String']['output']>;
  level: LogLevel;
  message: Scalars['String']['output'];
  timestamp: Scalars['DateTime']['output'];
};

export type LogDeleteOutput = {
  __typename?: 'LogDeleteOutput';
  deleted: Scalars['Int']['output'];
};

export type LogFileInfo = {
  __typename?: 'LogFileInfo';
  modified: Scalars['String']['output'];
  path: Scalars['String']['output'];
  size: Scalars['Int']['output'];
};

export type LogFilterInput = {
  _and?: InputMaybe<Array<LogFilterInput>>;
  _not?: InputMaybe<Array<LogFilterInput>>;
  _or?: InputMaybe<Array<LogFilterInput>>;
  jobId?: InputMaybe<FieldFilterString>;
  level?: InputMaybe<FieldFilterString>;
};

export enum LogLevel {
  Debug = 'DEBUG',
  Error = 'ERROR',
  Info = 'INFO',
  Warn = 'WARN'
}

export type LogModelOrderBy = {
  direction: OrderDirection;
  field: LogModelOrdering;
};

export enum LogModelOrdering {
  Context = 'CONTEXT',
  Id = 'ID',
  JobId = 'JOB_ID',
  Level = 'LEVEL',
  Message = 'MESSAGE',
  Timestamp = 'TIMESTAMP'
}

/** A potential match from an external provider */
export type MatchCandidate = {
  __typename?: 'MatchCandidate';
  /** Confidence score (0.0 - 1.0) */
  confidence: Scalars['Float']['output'];
  /** Factors that contributed to the confidence score */
  confidenceFactors: Array<ConfidenceFactor>;
  /** External ID on the provider's system */
  externalId: Scalars['String']['output'];
  metadata: ExternalMetadata;
  /** The provider this match came from */
  provider: Scalars['String']['output'];
};

export type Media = {
  __typename?: 'Media';
  analysisData?: Maybe<MediaAnalysisData>;
  /** The timestamp of the creation of the media */
  createdAt: Scalars['DateTime']['output'];
  /** The timestamp of when the media was **soft** deleted. This will act like a trash bin. */
  deletedAt?: Maybe<Scalars['DateTime']['output']>;
  /** If the media is an epub, this will return the parsed epub data from the file */
  ebook?: Maybe<Epub>;
  /** The extension of the media file, excluding the leading period */
  extension: Scalars['String']['output'];
  /**
   * A Stump-specific hash of the media file. This is used as a secondary identifier for the media, primarily
   * in aiding in the identification of duplicate media files
   */
  hash?: Maybe<Scalars['String']['output']>;
  /** The unique identifier for the media */
  id: Scalars['String']['output'];
  /** Whether the media is marked as a favorite by the current user */
  isFavorite: Scalars['Boolean']['output'];
  isOneshot: Scalars['Boolean']['output'];
  /**
   * A hash of the media file that adheres to the KoReader hash algorithm. This is used to identify
   * books from the KoReader application so progress can be synced between the two applications
   */
  koreaderHash?: Maybe<Scalars['String']['output']>;
  library: Library;
  libraryConfig: LibraryConfig;
  libraryId: Scalars['String']['output'];
  metadata?: Maybe<MediaMetadata>;
  /**
   * The timestamp of when the underlying file was last modified on disk. This will only be set if
   * a timestamp can be retrieved from the filesystem
   */
  modifiedAt?: Maybe<Scalars['DateTime']['output']>;
  /** The name of the media, derived from the filename and excluding the extension */
  name: Scalars['String']['output'];
  /**
   * The next media in the series, ordered by metadata series position
   * when available and falling back to name if not
   */
  nextInSeries: PaginatedMediaResponse;
  /** The number of pages in the media, if applicable. Will be -1 for certain media types */
  pages: Scalars['Int']['output'];
  /** The path of the underlying media file on disk */
  path: Scalars['String']['output'];
  readHistory: Array<ReadthroughRecord>;
  readProgress?: Maybe<ResumeReadingCursor>;
  /** The IDs of the current user's reading lists with this book */
  readingListIds: Array<Scalars['String']['output']>;
  /**
   * The reading timeline for the book for the current user. Will be `None` if the user has not
   * read the book.
   *
   * Note: This is not paginated and loads the entire timeline at once for the user, so it will be expensive
   * if selected in a query for N number of books
   */
  readingTimeline?: Maybe<BookReadingTimeline>;
  /**
   * The path to the media file **relative** to the library path. This is only useful for
   * displaying a truncated path when in the context of a library, e.g. limited space
   * on a mobile device.
   */
  relativeLibraryPath: Scalars['String']['output'];
  /**
   * The resolved name of the media, which will prioritize the title pulled from
   * metatadata, if available, and fallback to the name derived from the file name
   */
  resolvedName: Scalars['String']['output'];
  /** The series the media belongs to */
  series?: Maybe<Series>;
  /**
   * The unique identifier of the series that the media belongs to. While this is nullable, it is
   * expected that all media will belong to a series
   */
  seriesId?: Maybe<Scalars['String']['output']>;
  seriesPosition?: Maybe<Scalars['Int']['output']>;
  /** The size of the media file in bytes */
  size: Scalars['Int']['output'];
  /**
   * The status of the media. This is used to determine if the media is available for reading (i.e.,
   * if it is available on disk)
   */
  status: FileStatus;
  /** The tags associated with the media */
  tags: Array<Tag>;
  /**
   * A reference to the thumbnail image for the media. This will be a fully
   * qualified URL to the image.
   */
  thumbnail: ImageRef;
  /** The metadata for the thumbnail image of the media */
  thumbnailMeta?: Maybe<ImageMetadata>;
  /** The path to the thumbnail image of the media on disk */
  thumbnailPath?: Maybe<Scalars['String']['output']>;
  /** The timestamp of the last time the media was updated. This will be set during creation, as well */
  updatedAt?: Maybe<Scalars['DateTime']['output']>;
};


export type MediaNextInSeriesArgs = {
  pagination?: Pagination;
};


export type MediaReadingTimelineArgs = {
  order?: OrderDirection;
};

export type MediaAnalysisData = {
  __typename?: 'MediaAnalysisData';
  contentTypes: Array<Scalars['String']['output']>;
  dimensions: Array<PageDimension>;
};

export type MediaAnnotation = {
  __typename?: 'MediaAnnotation';
  annotationText?: Maybe<Scalars['String']['output']>;
  createdAt: Scalars['DateTime']['output'];
  id: Scalars['String']['output'];
  locator: ReadiumLocator;
  mediaId: Scalars['String']['output'];
  /**
   * The ID of the session which this annotation was created in. Realistically, when using a Stump
   * reader this should always be set
   */
  sessionId?: Maybe<Scalars['Int']['output']>;
  /** How the annotated text is drawn */
  style: AnnotationStyle;
  updatedAt: Scalars['DateTime']['output'];
  userId: Scalars['String']['output'];
};

/**
 * A media annotation represents a highlight (or underline, strikethrough, invert) and/or
 * a note. Annotations are private to the user who made them
 */
export type MediaAnnotationModel = {
  __typename?: 'MediaAnnotationModel';
  annotationText?: Maybe<Scalars['String']['output']>;
  createdAt: Scalars['DateTime']['output'];
  id: Scalars['String']['output'];
  locator: ReadiumLocator;
  mediaId: Scalars['String']['output'];
  /**
   * The ID of the session which this annotation was created in. Realistically, when using a Stump
   * reader this should always be set
   */
  sessionId?: Maybe<Scalars['Int']['output']>;
  /** How the annotated text is drawn */
  style: AnnotationStyle;
  updatedAt: Scalars['DateTime']['output'];
  userId: Scalars['String']['output'];
};

/** A description an online source has for a book */
export type MediaDescriptionCandidate = {
  __typename?: 'MediaDescriptionCandidate';
  /** `HARDCOVER`, `OPEN_LIBRARY` or `GOOGLE_BOOKS` */
  source: IntakeSource;
  text: Scalars['String']['output'];
  words: Scalars['Int']['output'];
};

/** What a description search did for a book */
export enum MediaDescriptionStatus {
  /** Descriptions were found (`candidates`, best first) */
  Found = 'FOUND',
  /** Not searched: the book has a summary (with `onlyMissing`) */
  HasSummary = 'HAS_SUMMARY',
  /** Not searched: its summary is locked (someone set or cleared it by hand) */
  Locked = 'LOCKED',
  /** No book with that id or path that the user can see */
  NotFound = 'NOT_FOUND',
  /** Hardcover and Open Library answered, and no source has a description for the book */
  NoDescription = 'NO_DESCRIPTION',
  /**
   * Nothing found while Hardcover or Open Library couldn't answer (a rate limit, an
   * error): ask again later
   */
  RetryLater = 'RETRY_LATER'
}

/**
 * The descriptions found online for a book. Nothing is saved: put one into the book's
 * summary with `editMediaMetadata` / `editMediaMetadataBatch` (file, history, lock)
 */
export type MediaDescriptions = {
  __typename?: 'MediaDescriptions';
  /** Hardcover's first, then Open Library's and Google Books' (thin ones last), one per text */
  candidates: Array<MediaDescriptionCandidate>;
  /** The book's id (the id asked for when there is no such book) */
  mediaId?: Maybe<Scalars['String']['output']>;
  /**
   * How each online source answered: found a description (N words), found but no
   * description, no matching book, rate-limited, error, not configured
   */
  onlineLookups: Array<OnlineLookup>;
  /** The book's path as Stump stores it (the path asked for when there is no such book) */
  path?: Maybe<Scalars['String']['output']>;
  searchedAuthors: Array<Scalars['String']['output']>;
  /**
   * The title and authors searched, when the book's own looked like junk (from its title
   * page, or its file name for a missing title)
   */
  searchedTitle?: Maybe<Scalars['String']['output']>;
  status: MediaDescriptionStatus;
  /** The summary the book has now */
  summary?: Maybe<Scalars['String']['output']>;
};

export type MediaFilterInput = {
  _and?: InputMaybe<Array<MediaFilterInput>>;
  _not?: InputMaybe<Array<MediaFilterInput>>;
  _or?: InputMaybe<Array<MediaFilterInput>>;
  createdAt?: InputMaybe<NumericFilterDateTime>;
  extension?: InputMaybe<FieldFilterString>;
  id?: InputMaybe<FieldFilterString>;
  libraryId?: InputMaybe<FieldFilterString>;
  libraryType?: InputMaybe<ComputedFilterLibraryType>;
  metadata?: InputMaybe<MediaMetadataFilterInput>;
  name?: InputMaybe<FieldFilterString>;
  pages?: InputMaybe<NumericFilterI32>;
  path?: InputMaybe<FieldFilterString>;
  readingStatus?: InputMaybe<ComputedFilterReadingStatus>;
  /**
   * Free-text search: every word must appear in the file name or path, title, series,
   * writers, publisher, genres or tags, ignoring case and accents
   */
  search?: InputMaybe<Scalars['String']['input']>;
  series?: InputMaybe<SeriesFilterInput>;
  seriesId?: InputMaybe<FieldFilterString>;
  size?: InputMaybe<NumericFilterI64>;
  status?: InputMaybe<FieldFilterFileStatus>;
  tags?: InputMaybe<FieldFilterString>;
  updatedAt?: InputMaybe<NumericFilterDateTime>;
};

/** Tags proposed for a book already in the library, and how its current ones fit */
export type MediaGenreSuggestions = {
  __typename?: 'MediaGenreSuggestions';
  /** The book's current tags, judged against the same evidence */
  current: Array<CurrentGenreFit>;
  /** Whether an online source found the book (a failed lookup isn't one) */
  lookedUpOnline: Scalars['Boolean']['output'];
  /**
   * How each online source answered (empty when they weren't asked): found with N
   * subjects, no subjects, no match, rate-limited (Google Books without an API key), error
   */
  onlineLookups: Array<OnlineLookup>;
  /** Books of the library it was compared with */
  similarBooks: Array<IntakeSimilarBook>;
  /** Subjects found online */
  subjects: Array<Scalars['String']['output']>;
  /**
   * The library's tags that fit the book, best first (`selected` marks the default
   * choice)
   */
  suggestions: Array<IntakeGenreSuggestion>;
};

export type MediaMetadata = {
  __typename?: 'MediaMetadata';
  ageRating?: Maybe<Scalars['Int']['output']>;
  characters: Array<Scalars['String']['output']>;
  colorists: Array<Scalars['String']['output']>;
  coverArtists: Array<Scalars['String']['output']>;
  day?: Maybe<Scalars['Int']['output']>;
  editors: Array<Scalars['String']['output']>;
  format?: Maybe<Scalars['String']['output']>;
  genres: Array<Scalars['String']['output']>;
  id: Scalars['Int']['output'];
  identifierAmazon?: Maybe<Scalars['String']['output']>;
  identifierCalibre?: Maybe<Scalars['String']['output']>;
  identifierGoogle?: Maybe<Scalars['String']['output']>;
  identifierIsbn?: Maybe<Scalars['String']['output']>;
  identifierMobiAsin?: Maybe<Scalars['String']['output']>;
  identifierUuid?: Maybe<Scalars['String']['output']>;
  inkers: Array<Scalars['String']['output']>;
  language?: Maybe<Scalars['String']['output']>;
  letterers: Array<Scalars['String']['output']>;
  links: Array<Scalars['String']['output']>;
  lockedFields: Array<MetadataField>;
  mediaId?: Maybe<Scalars['String']['output']>;
  /** The external ID on the metadata provider's system */
  metadataExternalId?: Maybe<Scalars['String']['output']>;
  /** The external metadata provider that supplied this metadata (e.g., "HARDCOVER") */
  metadataSource?: Maybe<Scalars['String']['output']>;
  month?: Maybe<Scalars['Int']['output']>;
  notes?: Maybe<Scalars['String']['output']>;
  number?: Maybe<Scalars['Decimal']['output']>;
  pageCount?: Maybe<Scalars['Int']['output']>;
  pencillers: Array<Scalars['String']['output']>;
  publisher?: Maybe<Scalars['String']['output']>;
  /**
   * A community/reader rating from 0 to 5 (e.g. EPUB `calibre:rating` or ComicInfo
   * `CommunityRating`)
   */
  rating?: Maybe<Scalars['Decimal']['output']>;
  series?: Maybe<Scalars['String']['output']>;
  seriesGroup?: Maybe<Scalars['String']['output']>;
  storyArc?: Maybe<Scalars['String']['output']>;
  storyArcNumber?: Maybe<Scalars['Decimal']['output']>;
  summary?: Maybe<Scalars['String']['output']>;
  teams: Array<Scalars['String']['output']>;
  title?: Maybe<Scalars['String']['output']>;
  titleSort?: Maybe<Scalars['String']['output']>;
  volume?: Maybe<Scalars['Int']['output']>;
  writers: Array<Scalars['String']['output']>;
  year?: Maybe<Scalars['Int']['output']>;
};

/**
 * One book of `editMediaMetadataBatch`: named by `id` or by `path` (as Stump stores it; both
 * may be given, then they must agree), with the values to set
 */
export type MediaMetadataBatchInput = {
  id?: InputMaybe<Scalars['ID']['input']>;
  input: MediaMetadataInput;
  path?: InputMaybe<Scalars['String']['input']>;
};

/** One book of `editMediaMetadataBatch`, in the order sent */
export type MediaMetadataBatchItemResult = {
  __typename?: 'MediaMetadataBatchItemResult';
  /** The fields changed (or, for a dry run, that would change), with old and new values */
  changes: Array<MetadataEditChange>;
  /** Why it wasn't saved, or why the file kept its old values */
  errors: Array<Scalars['String']['output']>;
  /** Where the changes went (or, for a dry run, would go) */
  file?: Maybe<MetadataEditFile>;
  /** Its position in `inputs` */
  index: Scalars['Int']['output'];
  /** The book found, if any */
  mediaId?: Maybe<Scalars['String']['output']>;
  path?: Maybe<Scalars['String']['output']>;
  /** The id and path the item was sent with */
  requestedId?: Maybe<Scalars['String']['output']>;
  requestedPath?: Maybe<Scalars['String']['output']>;
  /** The save in the book's edit history (revert it with `revertMediaMetadataEdit`) */
  saveId?: Maybe<Scalars['String']['output']>;
  status: MetadataBatchItemStatus;
};

/** How `editMediaMetadataBatch` saves */
export type MediaMetadataBatchOptions = {
  /** Only report what would change: nothing is saved or written */
  dryRun?: Scalars['Boolean']['input'];
  /** Write the changes into the book files (else they stay in Stump only, locked) */
  writeToFile?: Scalars['Boolean']['input'];
};

/** The result of `editMediaMetadataBatch` */
export type MediaMetadataBatchResult = {
  __typename?: 'MediaMetadataBatchResult';
  dryRun: Scalars['Boolean']['output'];
  failed: Scalars['Int']['output'];
  items: Array<MediaMetadataBatchItemResult>;
  notFound: Scalars['Int']['output'];
  /**
   * How many books the targeted rescan job queued at the end reads again (the ones whose
   * files were rewritten; 0: no rescan was needed)
   */
  rescanBooks: Scalars['Int']['output'];
  saved: Scalars['Int']['output'];
  unchanged: Scalars['Int']['output'];
  /** Items that would be saved (dry run) */
  wouldSave: Scalars['Int']['output'];
};

export type MediaMetadataFilterInput = {
  _and?: InputMaybe<Array<MediaMetadataFilterInput>>;
  _not?: InputMaybe<Array<MediaMetadataFilterInput>>;
  _or?: InputMaybe<Array<MediaMetadataFilterInput>>;
  /**
   * `true` keeps adult (18+) books only, `false` keeps everything else, including
   * books without an age rating
   */
  adult?: InputMaybe<Scalars['Boolean']['input']>;
  ageRating?: InputMaybe<NumericFilterI32>;
  characters?: InputMaybe<FieldFilterString>;
  colorists?: InputMaybe<FieldFilterString>;
  coverArtists?: InputMaybe<FieldFilterString>;
  day?: InputMaybe<NumericFilterI32>;
  editors?: InputMaybe<FieldFilterString>;
  genres?: InputMaybe<FieldFilterString>;
  inkers?: InputMaybe<FieldFilterString>;
  language?: InputMaybe<FieldFilterString>;
  letterers?: InputMaybe<FieldFilterString>;
  links?: InputMaybe<FieldFilterString>;
  month?: InputMaybe<NumericFilterI32>;
  pencillers?: InputMaybe<FieldFilterString>;
  publisher?: InputMaybe<FieldFilterString>;
  /** The community/reader rating, from 0 to 5 */
  rating?: InputMaybe<NumericFilterF32>;
  series?: InputMaybe<FieldFilterString>;
  summary?: InputMaybe<FieldFilterString>;
  teams?: InputMaybe<FieldFilterString>;
  title?: InputMaybe<FieldFilterString>;
  writers?: InputMaybe<FieldFilterString>;
  year?: InputMaybe<NumericFilterI32>;
};

export type MediaMetadataInput = {
  ageRating?: InputMaybe<Scalars['Int']['input']>;
  characters?: InputMaybe<Array<Scalars['String']['input']>>;
  colorists?: InputMaybe<Array<Scalars['String']['input']>>;
  coverArtists?: InputMaybe<Array<Scalars['String']['input']>>;
  day?: InputMaybe<Scalars['Int']['input']>;
  editors?: InputMaybe<Array<Scalars['String']['input']>>;
  format?: InputMaybe<Scalars['String']['input']>;
  genres?: InputMaybe<Array<Scalars['String']['input']>>;
  identifierAmazon?: InputMaybe<Scalars['String']['input']>;
  identifierCalibre?: InputMaybe<Scalars['String']['input']>;
  identifierGoogle?: InputMaybe<Scalars['String']['input']>;
  identifierIsbn?: InputMaybe<Scalars['String']['input']>;
  identifierMobiAsin?: InputMaybe<Scalars['String']['input']>;
  identifierUuid?: InputMaybe<Scalars['String']['input']>;
  inkers?: InputMaybe<Array<Scalars['String']['input']>>;
  language?: InputMaybe<Scalars['String']['input']>;
  letterers?: InputMaybe<Array<Scalars['String']['input']>>;
  links?: InputMaybe<Array<Scalars['String']['input']>>;
  month?: InputMaybe<Scalars['Int']['input']>;
  notes?: InputMaybe<Scalars['String']['input']>;
  number?: InputMaybe<Scalars['Decimal']['input']>;
  pageCount?: InputMaybe<Scalars['Int']['input']>;
  pencillers?: InputMaybe<Array<Scalars['String']['input']>>;
  publisher?: InputMaybe<Scalars['String']['input']>;
  /**
   * The book's rating, 0 to 5 (decimals allowed). EPUBs get calibre's half stars and a
   * precise `bcurator:rating` meta, PDFs a `bcurator:rating=` keyword field, CBZs ComicInfo's
   * `CommunityRating`
   */
  rating?: InputMaybe<Scalars['Decimal']['input']>;
  series?: InputMaybe<Scalars['String']['input']>;
  seriesGroup?: InputMaybe<Scalars['String']['input']>;
  storyArc?: InputMaybe<Scalars['String']['input']>;
  storyArcNumber?: InputMaybe<Scalars['Decimal']['input']>;
  summary?: InputMaybe<Scalars['String']['input']>;
  teams?: InputMaybe<Array<Scalars['String']['input']>>;
  title?: InputMaybe<Scalars['String']['input']>;
  titleSort?: InputMaybe<Scalars['String']['input']>;
  volume?: InputMaybe<Scalars['Int']['input']>;
  writers?: InputMaybe<Array<Scalars['String']['input']>>;
  year?: InputMaybe<Scalars['Int']['input']>;
};

export enum MediaMetadataModelOrdering {
  AgeRating = 'AGE_RATING',
  Characters = 'CHARACTERS',
  Colorists = 'COLORISTS',
  CoverArtists = 'COVER_ARTISTS',
  Day = 'DAY',
  Editors = 'EDITORS',
  Format = 'FORMAT',
  Genres = 'GENRES',
  Id = 'ID',
  IdentifierAmazon = 'IDENTIFIER_AMAZON',
  IdentifierCalibre = 'IDENTIFIER_CALIBRE',
  IdentifierGoogle = 'IDENTIFIER_GOOGLE',
  IdentifierIsbn = 'IDENTIFIER_ISBN',
  IdentifierMobiAsin = 'IDENTIFIER_MOBI_ASIN',
  IdentifierUuid = 'IDENTIFIER_UUID',
  Inkers = 'INKERS',
  Language = 'LANGUAGE',
  Letterers = 'LETTERERS',
  Links = 'LINKS',
  LockedFields = 'LOCKED_FIELDS',
  MediaId = 'MEDIA_ID',
  MetadataExternalId = 'METADATA_EXTERNAL_ID',
  MetadataSource = 'METADATA_SOURCE',
  Month = 'MONTH',
  Notes = 'NOTES',
  Number = 'NUMBER',
  PageCount = 'PAGE_COUNT',
  Pencillers = 'PENCILLERS',
  Publisher = 'PUBLISHER',
  Rating = 'RATING',
  Series = 'SERIES',
  SeriesGroup = 'SERIES_GROUP',
  StoryArc = 'STORY_ARC',
  StoryArcNumber = 'STORY_ARC_NUMBER',
  Summary = 'SUMMARY',
  Teams = 'TEAMS',
  Title = 'TITLE',
  TitleSort = 'TITLE_SORT',
  Volume = 'VOLUME',
  Writers = 'WRITERS',
  Year = 'YEAR'
}

export type MediaMetadataOrderByField = {
  direction: OrderDirection;
  field: MediaMetadataModelOrdering;
};

export type MediaMetadataOverview = {
  __typename?: 'MediaMetadataOverview';
  characters: Array<Scalars['String']['output']>;
  colorists: Array<Scalars['String']['output']>;
  coverArtists: Array<Scalars['String']['output']>;
  editors: Array<Scalars['String']['output']>;
  genres: Array<Scalars['String']['output']>;
  inkers: Array<Scalars['String']['output']>;
  /**
   * The two-letter codes of the languages in use (`en`, `EN`, `en-US` and `eng` are
   * all reported as `en`), matching how the `language` filter compares values
   */
  languages: Array<Scalars['String']['output']>;
  letterers: Array<Scalars['String']['output']>;
  pencillers: Array<Scalars['String']['output']>;
  publishers: Array<Scalars['String']['output']>;
  series: Array<Scalars['String']['output']>;
  teams: Array<Scalars['String']['output']>;
  writers: Array<Scalars['String']['output']>;
};

/**
 * A manual override for searching metadata providers for a single media item. When
 * provided, the caller's fields take precedence over whatever is already stored on the
 * media, and the search is restricted to a single provider if one is given.
 */
export type MediaMetadataSearchInput = {
  author?: InputMaybe<Scalars['String']['input']>;
  /**
   * The volume ID to search within, which will swap to a more precise lookup if provided alongside
   * `number`
   */
  comicVineVolumeId?: InputMaybe<Scalars['String']['input']>;
  isbn?: InputMaybe<Scalars['String']['input']>;
  limit?: InputMaybe<Scalars['Int']['input']>;
  /** The issue number (for comics/manga) */
  number?: InputMaybe<Scalars['Float']['input']>;
  /**
   * Restrict the search to this provider only. If omitted, all enabled providers
   * configured for the media's library type are searched.
   */
  provider?: InputMaybe<MetadataProvider>;
  title?: InputMaybe<Scalars['String']['input']>;
  year?: InputMaybe<Scalars['Int']['input']>;
};

export enum MediaModelOrdering {
  CreatedAt = 'CREATED_AT',
  DeletedAt = 'DELETED_AT',
  Extension = 'EXTENSION',
  Hash = 'HASH',
  Id = 'ID',
  IsOneshot = 'IS_ONESHOT',
  KoreaderHash = 'KOREADER_HASH',
  ModifiedAt = 'MODIFIED_AT',
  Name = 'NAME',
  Pages = 'PAGES',
  Path = 'PATH',
  SeriesId = 'SERIES_ID',
  Size = 'SIZE',
  Status = 'STATUS',
  ThumbnailMeta = 'THUMBNAIL_META',
  ThumbnailPath = 'THUMBNAIL_PATH',
  UpdatedAt = 'UPDATED_AT'
}

export type MediaOrderBy =
  { media: MediaOrderByField; metadata?: never; }
  |  { media?: never; metadata: MediaMetadataOrderByField; };

export type MediaOrderByField = {
  direction: OrderDirection;
  field: MediaModelOrdering;
};

/** A path asked for by `mediaByPaths`, and the book there */
export type MediaPathMatch = {
  __typename?: 'MediaPathMatch';
  media?: Maybe<Media>;
  path: Scalars['String']['output'];
};

export type MediaProgressInput =
  { epub: EpubProgressInput; paged?: never; }
  |  { epub?: never; paged: PagedProgressInput; };

/** How to merge external metadata values onto existing entity metadata */
export enum MergeStrategy {
  /** FillGaps and merge/dedupe for array fields */
  FillAndMergeLists = 'FILL_AND_MERGE_LISTS',
  /** Only populate fields that are currently nullish */
  FillGaps = 'FILL_GAPS',
  /** Overwrite existing values with (truthy) external data */
  PreferExternal = 'PREFER_EXTERNAL',
  /** PreferExternal for scalars, merge/dedupe for array fields */
  PreferExternalAndMergeLists = 'PREFER_EXTERNAL_AND_MERGE_LISTS'
}

/** What happened to one book of a batch edit */
export enum MetadataBatchItemStatus {
  /** Not saved: see `errors` */
  Failed = 'FAILED',
  /** No book with that id or path that the caller can see (removed books included) */
  NotFound = 'NOT_FOUND',
  /** Saved (see `file` for whether the book file has it) */
  Saved = 'SAVED',
  /** Nothing to change: the book already had these values */
  Unchanged = 'UNCHANGED',
  /** Dry run: these changes would be saved */
  WouldSave = 'WOULD_SAVE'
}

/** One field changed by a save */
export type MetadataEditChange = {
  __typename?: 'MetadataEditChange';
  /** The field's camelCase name (`genres`, `writers`, ...) */
  field: Scalars['String']['output'];
  newValue?: Maybe<Scalars['String']['output']>;
  /** The value before (lists comma-separated) */
  oldValue?: Maybe<Scalars['String']['output']>;
  /** Whether the book file carries the new value (else it was kept in Stump only) */
  writtenToFile: Scalars['Boolean']['output'];
};

/** Where the changes of a metadata edit went */
export type MetadataEditFile = {
  __typename?: 'MetadataEditFile';
  /** The changed fields (camelCase) now in the book file */
  fileFields: Array<Scalars['String']['output']>;
  /** The locks that keep the edited fields through rescans */
  locked: Array<MetadataField>;
  /** Why the file was not written, or a note about it */
  message?: Maybe<Scalars['String']['output']>;
  status: MetadataFileWriteStatus;
  /** The changed fields kept in Stump only */
  stumpOnlyFields: Array<Scalars['String']['output']>;
};

/** The result of saving a book's metadata */
export type MetadataEditResult = {
  __typename?: 'MetadataEditResult';
  file: MetadataEditFile;
  media: Media;
  /** The save in the book's edit history (none when nothing changed) */
  saveId?: Maybe<Scalars['String']['output']>;
  /** For a revert: the fields left as they are because they changed again since */
  skippedFields: Array<Scalars['String']['output']>;
};

/** One save of a book's metadata edit history */
export type MetadataEditSave = {
  __typename?: 'MetadataEditSave';
  changes: Array<MetadataEditChange>;
  editedAt: Scalars['DateTime']['output'];
  /** The username of who saved it */
  editedBy?: Maybe<Scalars['String']['output']>;
  /** The save that reverted this one */
  revertedBy?: Maybe<Scalars['String']['output']>;
  /** The save this one reverted */
  revertsSaveId?: Maybe<Scalars['String']['output']>;
  saveId: Scalars['String']['output'];
};

/** The metadata fields which can be listed with their book counts */
export enum MetadataFacetField {
  Genres = 'GENRES',
  Language = 'LANGUAGE',
  Publisher = 'PUBLISHER',
  Series = 'SERIES',
  Writers = 'WRITERS'
}

export type MetadataFetchJobOutput = {
  __typename?: 'MetadataFetchJobOutput';
  /** Number of entities that were auto-applied */
  autoApplied: Scalars['Int']['output'];
  /** Number of entities that failed during fetch */
  failed: Scalars['Int']['output'];
  /** Number of entities where matches were found */
  matchesFound: Scalars['Int']['output'];
  /** Number of entities where no matches were found */
  noMatches: Scalars['Int']['output'];
  /** Number of entities that were rate-limited */
  rateLimited: Scalars['Int']['output'];
  /** Number of entities that were skipped (already have matches) */
  skipped: Scalars['Int']['output'];
  /** Total number of entities processed */
  totalProcessed: Scalars['Int']['output'];
};

export type MetadataFetchRecord = {
  __typename?: 'MetadataFetchRecord';
  acceptedMatchCandidate?: Maybe<MatchCandidate>;
  addedAt: Scalars['DateTime']['output'];
  id: Scalars['Int']['output'];
  matchCandidates: Array<MatchCandidate>;
  /** The media item associated with this fetch record, if any */
  media?: Maybe<Media>;
  mediaId?: Maybe<Scalars['String']['output']>;
  /**
   * The total number of raw hits reported by provider searches, across all
   * providers searched. Compare against `match_candidates.len()` to detect when
   * hits were dropped (e.g. a per-hit detail fetch failed after the initial search).
   */
  rawHits: Scalars['Int']['output'];
  /** The series associated with this fetch record, if any */
  series?: Maybe<Series>;
  seriesId?: Maybe<Scalars['String']['output']>;
  status: MetadataFetchStatus;
  updatedAt?: Maybe<Scalars['DateTime']['output']>;
};

/**
 * An identifer for specifying the target of a metadata fetch record query. I added
 * mostly for type safety and not annoyingly wrangling both media_id and series_id
 */
export type MetadataFetchRecordId =
  { media: Scalars['String']['input']; series?: never; }
  |  { media?: never; series: Scalars['String']['input']; };

/** Automatic metadata fetches from the metadata providers (Hardcover...) */
export type MetadataFetchSettings = {
  __typename?: 'MetadataFetchSettings';
  /**
   * Whether a scan asks the providers about the books it added, and the scheduled retries
   * run. Off keeps their daily quota for people's lookups; fetches someone starts still run
   */
  fetchAfterScan: Scalars['Boolean']['output'];
};

export enum MetadataFetchStatus {
  AwaitingReview = 'AWAITING_REVIEW',
  Failed = 'FAILED',
  Fetched = 'FETCHED',
  InProgress = 'IN_PROGRESS',
  Matched = 'MATCHED',
  NotStarted = 'NOT_STARTED',
  NoMatch = 'NO_MATCH',
  RateLimited = 'RATE_LIMITED'
}

/**
 * Represents a specific metadata field that can be locked or configured
 * for per-field merge strategies
 */
export enum MetadataField {
  AgeRating = 'AGE_RATING',
  Artists = 'ARTISTS',
  BookType = 'BOOK_TYPE',
  Characters = 'CHARACTERS',
  Colorists = 'COLORISTS',
  ComicId = 'COMIC_ID',
  ComicImage = 'COMIC_IMAGE',
  Cover = 'COVER',
  CoverArtists = 'COVER_ARTISTS',
  DescriptionFormatted = 'DESCRIPTION_FORMATTED',
  Editors = 'EDITORS',
  Format = 'FORMAT',
  Genres = 'GENRES',
  IdentifierAmazon = 'IDENTIFIER_AMAZON',
  IdentifierCalibre = 'IDENTIFIER_CALIBRE',
  IdentifierGoogle = 'IDENTIFIER_GOOGLE',
  IdentifierMobiAsin = 'IDENTIFIER_MOBI_ASIN',
  IdentifierUuid = 'IDENTIFIER_UUID',
  Imprint = 'IMPRINT',
  Inkers = 'INKERS',
  Isbn = 'ISBN',
  Language = 'LANGUAGE',
  Letterers = 'LETTERERS',
  Links = 'LINKS',
  MetaType = 'META_TYPE',
  Notes = 'NOTES',
  Number = 'NUMBER',
  PageCount = 'PAGE_COUNT',
  Pencillers = 'PENCILLERS',
  PublicationRun = 'PUBLICATION_RUN',
  Publisher = 'PUBLISHER',
  /** The 0-5 rating (a book's own, from its file or set by hand) */
  Rating = 'RATING',
  ReleaseDate = 'RELEASE_DATE',
  Series = 'SERIES',
  SeriesGroup = 'SERIES_GROUP',
  Status = 'STATUS',
  StoryArc = 'STORY_ARC',
  StoryArcNumber = 'STORY_ARC_NUMBER',
  Summary = 'SUMMARY',
  Tags = 'TAGS',
  Teams = 'TEAMS',
  Title = 'TITLE',
  TitleSort = 'TITLE_SORT',
  VolumeCount = 'VOLUME_COUNT',
  Writers = 'WRITERS',
  Year = 'YEAR'
}

/** A user-provided override value for a specific metadata field */
export type MetadataFieldOverride = {
  /** Which metadata field this override applies to */
  field: MetadataField;
  /** The value to set, using Json because I am lazy */
  value: Scalars['JSON']['input'];
};

/** Whether an edit reached the book file */
export enum MetadataFileWriteStatus {
  /** The file could not take the changes: they are kept in Stump only (and locked) */
  Failed = 'FAILED',
  /** None of the changes is something the file carries */
  NotNeeded = 'NOT_NEEDED',
  /** The save asked to keep the changes in Stump only (`writeToFile: false`) */
  Skipped = 'SKIPPED',
  /** The book file now carries the changed values */
  Written = 'WRITTEN'
}

/** The supported external metadata providers */
export enum MetadataProvider {
  /** ComicVine (https://comicvine.gamespot.com/api/) */
  ComicVine = 'COMIC_VINE',
  /** Hardcover (https://hardcover.app) */
  Hardcover = 'HARDCOVER'
}

export type MetadataProviderConfigModel = {
  __typename?: 'MetadataProviderConfigModel';
  apiTokenExpiresAt?: Maybe<Scalars['DateTime']['output']>;
  autoApplyConfig?: Maybe<Scalars['JSON']['output']>;
  createdAt: Scalars['DateTime']['output'];
  enabled: Scalars['Boolean']['output'];
  id: Scalars['Int']['output'];
  providerType: MetadataProvider;
  updatedAt?: Maybe<Scalars['DateTime']['output']>;
};

/**
 * An enum representing the different types of metadata resets that can occur,
 * which manifest differently depending on the context
 */
export enum MetadataResetImpact {
  /**
   * A reset which affects one or more books depending on context:
   * - When mutating a library, will reset all book metadata in all
   * library's series
   * - When mutating a series, will reset all its own books
   * - When mutating a book, will reset all its own metadata
   */
  Books = 'BOOKS',
  /**
   * A reset which affects multiple different entities (series, books) depending on context:
   * - When mutating a library, will reset all metadata in all library's series and books
   * - When mutating a series, will reset all its own metadata and books
   */
  Everything = 'EVERYTHING',
  /**
   * A reset which affects one or more series depending on context:
   * - When mutating a library, will reset all series metadata
   * - When mutating a series, will reset all its own metadata
   */
  Series = 'SERIES'
}

export type MetadataRetryConfigInput = {
  /** Which metadata fetch statuses to retry (e.g. RATE_LIMITED, FAILED) */
  statuses: Array<MetadataFetchStatus>;
};

export type MetadataValueCount = {
  __typename?: 'MetadataValueCount';
  count: Scalars['Int']['output'];
  value: Scalars['String']['output'];
};

export type MissingEntity = {
  __typename?: 'MissingEntity';
  id: Scalars['String']['output'];
  name: Scalars['String']['output'];
  path?: Maybe<Scalars['String']['output']>;
  type: MissingEntityType;
};

export enum MissingEntityType {
  Book = 'BOOK',
  Library = 'LIBRARY',
  Series = 'SERIES'
}

export type Mutation = {
  __typename?: 'Mutation';
  /** Accept the top-ranked candidate for all pending metadata matches */
  acceptAllPendingMatches: Scalars['Int']['output'];
  /**
   * a more focused version of `update_media_progress` that splices the history so that
   * any sessions after the ancestor_session_id are deleted in favor of the input
   * provided. this should be called when resolving local vs remote progress conflicts, where
   * the user has chosen to keep their local progress and discard the remote progress beyond
   * the ancestor session (i.e., the last session that both local and remote progress share)
   */
  acceptLocalProgress: ReadingSession;
  /** Accept a match candidate and apply it to media metadata */
  acceptMediaMatch: MetadataFetchRecord;
  /** Accept a match candidate and apply it to the series metadata */
  acceptSeriesMatch: MetadataFetchRecord;
  /** Add a book to the club's queue */
  addBookToClub: BookClub;
  /**
   * Adds books to the end of one of the current user's reading lists. Books already in it
   * and books the user can't see are left out.
   */
  addBooksToReadingList: ReadingList;
  analyzeLibrary: Scalars['Boolean']['output'];
  analyzeMedia: Scalars['Boolean']['output'];
  analyzeSeries: Scalars['Boolean']['output'];
  /** Archive or unarchive a discussion (Moderator+) */
  archiveDiscussion: Scalars['Boolean']['output'];
  cancelJob: Scalars['Boolean']['output'];
  /**
   * Delete media and series from a library that match one of the following conditions:
   *
   * - A series that is missing from disk (status is not `Ready`)
   * - A media that is missing from disk (status is not `Ready`)
   * - A series that is not associated with any media (i.e., no media in the series)
   *
   * This operation will also remove any associated thumbnails of the deleted media and series.
   */
  cleanLibrary: CleanLibraryResponse;
  /**
   * Clear the stored directory mtimes for a library, forcing the next scan to do a
   * full walk
   */
  clearLibraryScannedDirectories: Scalars['Int']['output'];
  /** trashes current readthrough, if there is one */
  clearMediaProgress: Scalars['Boolean']['output'];
  /** Clear the scan history for a specific library */
  clearScanHistory: Scalars['Int']['output'];
  /**
   * trashes all completed readthroughs for all books in this series, preserving any active
   * readthroughs
   */
  clearSeriesReadingHistory: Scalars['Int']['output'];
  /** Mark the current book as completed */
  completeBook: BookClub;
  convertMedia: Scalars['Boolean']['output'];
  /**
   * Create an annotation (a highlight, underline, strikethrough or invert, with an
   * optional note)
   */
  createAnnotation: MediaAnnotation;
  createApiKey: CreatedApiKey;
  createBookClub: BookClub;
  createBookClubInvitation: BookClubInvitation;
  /** Creates a new member in the book club */
  createBookClubMember: BookClubMember;
  /** Create a bookmark for a user */
  createBookmark: Bookmark;
  /** Manually create a discussion for a book */
  createDiscussion: BookClubDiscussion;
  createEmailDevice: RegisteredEmailDevice;
  createEmailer: Emailer;
  /**
   * Create a new library with the provided configuration. If `scan_after_persist` is `true`,
   * the library will be scanned immediately after creation.
   */
  createLibrary: Library;
  createMetadataProvider: MetadataProviderConfigModel;
  createNotifier: Notifier;
  /**
   * Creates a reading list owned by the current user, with the given books (in order; books
   * the user can't see are left out). The ID is generated when omitted.
   *
   * # Returns
   *
   * A result containing the newly created reading list, or an error if creation failed.
   */
  createReadingList: ReadingList;
  createScheduledJob: ScheduledJob;
  createSmartList: SmartList;
  createSmartListView: SmartListView;
  /**
   * Returns a list containing the newly created tags, or an error if creation failed.
   *
   * If any of the tags already exist an error is returned.
   *
   * * `tags` - A non-empty list of tags to create.
   */
  createTags: Array<Tag>;
  createUser: User;
  /** Delete an annotation by ID, only if the user created it */
  deleteAnnotation: MediaAnnotation;
  deleteApiKey: Apikey;
  deleteBookClub: BookClub;
  /** Delete a bookmark by ID, only if the user created it */
  deleteBookmark: Bookmark;
  /** Delete a custom emoji */
  deleteCustomEmoji: Scalars['Boolean']['output'];
  deleteEmailDevice: RegisteredEmailDevice;
  deleteEmailer: Emailer;
  /** Deletes held files for good (their copies in Stump's trash folder and their rows) */
  deleteHeldFiles: HeldFilesDeleteResult;
  deleteJob: Scalars['Boolean']['output'];
  deleteJobHistory: DeleteJobHistory;
  deleteJobLogs: DeleteJobAssociatedLogs;
  /** Deletes a reading journal entry for the authenticated user */
  deleteJournalEntry: Scalars['Boolean']['output'];
  /**
   * Delete a library, including all associated media and series via cascading deletes. This
   * operation cannot be undone.
   */
  deleteLibrary: Library;
  deleteLibraryScanHistory: Library;
  deleteLibraryThumbnails: Scalars['Boolean']['output'];
  deleteLogFile: Scalars['Boolean']['output'];
  deleteLoginActivity: Scalars['Int']['output'];
  deleteLogs: LogDeleteOutput;
  /**
   * Removes a book from its library: it goes to *Deleted books* (soft delete: hidden
   * everywhere, the file stays on disk) until a library manager restores it
   * (`restoreDeletedBooks`) or it is deleted permanently, by hand or by the daily purge once
   * the retention is over. Requires the `REMOVE_BOOK` permission, which an admin grants per
   * user
   */
  deleteMedia: Media;
  /** trashes all completed readthroughs for the media */
  deleteMediaReadingHistory: Scalars['Int']['output'];
  /** Delete (soft delete) your own message */
  deleteMessage: BookClubDiscussionMessage;
  deleteMetadataProvider: MetadataProviderConfigModel;
  deleteNotifier: Notifier;
  /** Deletes a reading device owned by the authenticated user */
  deleteReadingDevice: Scalars['Boolean']['output'];
  /**
   * Deletes one of the current user's reading lists (not its books).
   *
   * # Returns
   *
   * A result containing the deleted reading list, or an error if deletion failed.
   */
  deleteReadingList: ReadingList;
  /**
   * Deletes a reading session for the authenticated user. Only the user who owns the session
   * may delete it
   */
  deleteReadingSession: Scalars['Boolean']['output'];
  deleteScheduledJob: Scalars['Boolean']['output'];
  deleteSmartList: SmartList;
  deleteSmartListView: SmartListView;
  /**
   * Delete tags. Returns a list containing the deleted tags, or an error if deletion failed.
   *
   * * `tags` - A non-empty list of tags to delete.
   */
  deleteTags: Array<Tag>;
  deleteUser: User;
  /**
   * Delete the avatar for the authenticated viewer, or for any user if
   * called by a server owner (by passing `id`).
   */
  deleteUserAvatar: User;
  deleteUserSessions: Scalars['Int']['output'];
  /**
   * Save a book's metadata and write the changes into the book file (EPUB, PDF and CBZ
   * files carry title, authors, series, genres, summary, rating and more): the edited copy
   * is read back before it replaces the book, so a failed write leaves the file untouched
   * and the changes in Stump only. Every edited field is locked so rescans keep it, and
   * the save is recorded in the book's edit history (see `revertMediaMetadataEdit`).
   */
  editMediaMetadata: MetadataEditResult;
  /**
   * Edit many books' metadata in one call (for scripts: an API key with *Edit metadata*
   * works as a Bearer token). Each item names a book by `id` or by `path` (as Stump stores
   * it) and carries a `MediaMetadataInput` (omitted fields stay as they are, `null` clears
   * one). Every item goes through the same save as `editMediaMetadata` (book file written
   * and read back, database, locks, edit history), in order, one book at a time; an item
   * that fails doesn't stop the others. `dryRun` reports what would change without saving
   * anything; `writeToFile: false` keeps the changes in Stump only. Stump's own writes
   * don't set off a library scan: the books whose files were rewritten are read again by
   * one targeted rescan job at the end. At most 500 items per call.
   */
  editMediaMetadataBatch: MediaMetadataBatchResult;
  /** Edit your own message */
  editMessage: BookClubDiscussionMessage;
  /**
   * Deletes every book in *Deleted books* permanently (in one library, if given), like
   * `purgeDeletedBooks`
   */
  emptyDeletedBooks: DeletedBooksPurgeResult;
  extendReadingSession: ReadingSession;
  favoriteMedia: Media;
  favoriteSeries: Series;
  /** Start a job which will search external metadata providers */
  fetchLibraryMetadata: Scalars['Boolean']['output'];
  /**
   * Search external metadata providers for a media item and return the resulting fetch
   * record with its match candidates. When `search` is provided, the caller's fields
   * take precedence over the media's stored metadata, the search can be restricted to a
   * single provider, and auto-apply is skipped so the caller can review candidates before
   * anything is written.
   */
  fetchMediaMetadata: MetadataFetchRecord;
  /** Search external metadata providers for a series and return match candidates */
  fetchSeriesMetadata: Array<MatchCandidate>;
  /**
   * marks current readthrough as complete:
   * - if no current readthrough, creates one
   * - if `dnf` is true, it will mark the readthrough as such
   */
  finishMediaProgress: Scalars['Boolean']['output'];
  /** marks all books in the series as finished */
  finishSeriesProgress: Scalars['Int']['output'];
  generateLibraryThumbnails: Scalars['Boolean']['output'];
  /**
   * Files ready items into their library. Items that fail keep their state and get an
   * `error`; the others become `FILED`.
   */
  intakeAccept: Array<IntakeItem>;
  /** Throws items away with their staged files. Returns how many were discarded. */
  intakeDiscard: Scalars['Int']['output'];
  /** Analyses an item again (e.g. after a failure, or to look it up online) */
  intakeReanalyse: IntakeItem;
  /** Saves the uploader's review of an item (omitted fields keep their value) */
  intakeReview: IntakeItem;
  /**
   * Uploads books into the user's inbox for a library. Each book (or each folder / .zip
   * of volumes) becomes an item that is analysed right away; poll `intakeItems`.
   */
  intakeUpload: Array<IntakeItem>;
  /** Deletes the membership of the caller to the target book club */
  leaveBookClub: BookClubMember;
  /** Lock or unlock a discussion (Moderator+) */
  lockDiscussion: Scalars['Boolean']['output'];
  patchEmailDevice: RegisteredEmailDevice;
  patchLibrary: Library;
  patchLibraryConfig: LibraryConfig;
  /**
   * Updates a reading session for the authenticated user, exposing a small
   * set of fields which are editable
   */
  patchReadingSession: ReadingSession;
  /** Pin or unpin a message (Moderator+) */
  pinMessage: Scalars['Boolean']['output'];
  processLibraryThumbnails: Scalars['Boolean']['output'];
  /**
   * Deletes books in *Deleted books* permanently: the book's data and its file (and the
   * originals Stump holds for it) are deleted, for good. A book whose file or data can't be
   * deleted stays in Deleted books; one whose file is already gone only has its data deleted
   */
  purgeDeletedBooks: DeletedBooksPurgeResult;
  /**
   * Puts a held file back in its book's place (the original of a book post-processing
   * replaced): the book's current file is held in turn, so this can be undone the same way
   * until the retention is over
   */
  putBackHeldFile: HeldFilePutBack;
  /** Reject all pending metadata matches, setting their status to NoMatch */
  rejectAllPendingMatches: Scalars['Int']['output'];
  /** Reject the current match candidates for a media item */
  rejectMediaMatch: MetadataFetchRecord;
  /** Reject the current match candidates for a series */
  rejectSeriesMatch: MetadataFetchRecord;
  /** Removes a member from the book club */
  removeBookClubMember: BookClubMember;
  /** Removes books from one of the current user's reading lists */
  removeBooksFromReadingList: ReadingList;
  /** Remove your own suggestion (only before it's resolved) */
  removeSuggestion: BookClubBookSuggestion;
  /**
   * Rename a tag. Returns the updated tag, or an error if the tag was not found or the new
   * name already exists.
   */
  renameTag: Tag;
  /** Reorder uncompleted books in the club's queue. Completed books cannot be reordered since they are effectively archived */
  reorderBooks: BookClub;
  /** resets the elapsed seconds for all reading sessions in the current readthrough, if there is one */
  resetElapsedSeconds: Scalars['Boolean']['output'];
  resetLibraryMetadata: Library;
  resetSeriesMetadata: Series;
  respondToBookClubInvitation: BookClubInvitation;
  /**
   * Brings books in *Deleted books* back to their library exactly as they were (reading
   * progress, annotations, bookmarks, tags and smart lists never left). Returns how many came
   * back
   */
  restoreDeletedBooks: Scalars['Int']['output'];
  /** Brings back a book removed with `deleteMedia` */
  restoreMedia: Scalars['Boolean']['output'];
  /**
   * Revert a save of the book's edit history: the fields it changed get their old values
   * back (in the book file and in Stump), except the ones changed again since, which
   * are left as they are (`skippedFields`). The revert is recorded as a new save.
   */
  revertMediaMetadataEdit: MetadataEditResult;
  /**
   * Run post-processing on the whole library now: the 16-grey step (or its dry-run
   * estimate) and/or the PDF steps (queued for the PDF worker)
   */
  runPostProcessing: Scalars['Boolean']['output'];
  /** Enqueue a scan job for every library which the authenticated user has access to */
  scanAllLibraries: Scalars['Boolean']['output'];
  /**
   * Enqueue a scan job for a library. This will index the filesystem from the library's root path
   * and update the database accordingly.
   */
  scanLibrary: Scalars['Boolean']['output'];
  scanSeries: Scalars['Boolean']['output'];
  sendAttachmentEmail: SendAttachmentEmailOutput;
  /** Send a message in a discussion */
  sendMessage: BookClubDiscussionMessage;
  /** Bulk-set locked metadata fields for all media metadata in a library */
  setLibraryMediaLockedFields: Scalars['Int']['output'];
  /** Bulk-set locked metadata fields for all series metadata in a library */
  setLibrarySeriesLockedFields: Scalars['Int']['output'];
  /** Set the locked metadata fields for a media item */
  setMediaLockedFields: Media;
  /**
   * Set the tags for a media item. Creates any tags that don't exist yet, links new ones,
   * and unlinks removed ones. Returns the updated media item.
   */
  setMediaTags: Media;
  /** Set the locked metadata fields for a series */
  setSeriesLockedFields: Series;
  /**
   * Set the tags for a series. Creates any tags that don't exist yet, links new ones,
   * and unlinks removed ones. Returns the updated series.
   */
  setSeriesTags: Series;
  /** Suggest a book for the book club */
  suggestBook: BookClubBookSuggestion;
  /** Send a test email to verify the SMTP configuration is working */
  testEmailer: Scalars['Boolean']['output'];
  /**
   * Toggle a reaction on a message
   *
   * Returns true if the reaction was added, false if removed
   */
  toggleReaction: Scalars['Boolean']['output'];
  /** Toggle like on a suggestion */
  toggleSuggestionLike: Scalars['Boolean']['output'];
  /** Change an annotation's note and/or style, only if the user created it */
  updateAnnotation: MediaAnnotation;
  updateApiKey: Apikey;
  updateBookClub: BookClub;
  /** Rename a custom emoji */
  updateCustomEmoji: CustomEmoji;
  /**
   * How many days removed books stay in *Deleted books* before the daily purge deletes
   * them permanently (0: until deleted by hand)
   */
  updateDeletedBooksRetention: DeletedBooksSettings;
  updateEmailDevice: RegisteredEmailDevice;
  updateEmailer: Emailer;
  /** Replace the authenticated user's home sections */
  updateHomeArrangement: HomeArrangement;
  /**
   * Update an existing library with the provided configuration. If `scan_after_persist` is `true`,
   * the library will be scanned immediately after updating.
   * @deprecated Use `patchLibrary` instead
   */
  updateLibrary: Library;
  /** Update the emoji for a library */
  updateLibraryEmoji: Library;
  /**
   * Exclude users from a library, preventing them from seeing the library in the UI. This operates as a
   * full replacement of the excluded users list, so any users not included in the provided list will be
   * removed from the exclusion list if they were previously excluded.
   *
   * The server owner cannot be excluded from a library, nor can the user performing the action exclude
   * themselves.
   */
  updateLibraryExcludedUsers: Library;
  /** Changes how the intake files books into a library */
  updateLibraryIntakeSettings: LibraryIntakeSettings;
  /**
   * Change a library's post-processing settings. Turning a step on for new books processes
   * the books scans add from now on
   */
  updateLibraryPostProcessing: LibraryPostProcessing;
  /**
   * Update the thumbnail for a library. This will replace the existing thumbnail with the the one
   * associated with the provided input (book). If the book does not have a thumbnail, one
   * will be generated based on the library's thumbnail configuration.
   */
  updateLibraryThumbnail: Library;
  updateMediaMetadata: Media;
  updateMediaProgress: ReadingSession;
  /**
   * Update the thumbnail for a book. This will replace the existing thumbnail with the the one
   * associated with the provided input (book). If the book does not have a thumbnail, one
   * will be generated based on the library's thumbnail configuration.
   */
  updateMediaThumbnail: Media;
  /**
   * Turns the automatic metadata fetches on or off: what a scan asks the metadata
   * providers about the books it added, and the scheduled retries
   */
  updateMetadataFetchSettings: MetadataFetchSettings;
  updateMetadataProvider: MetadataProviderConfigModel;
  /**
   * Saves the signed-in user's reading settings (a JSON object as text, at most 1 MB),
   * replacing what was saved before
   */
  updateMyReaderSettings: Scalars['Boolean']['output'];
  updateNavigationArrangement: Arrangement;
  updateNavigationArrangementLock: Arrangement;
  updateNotifier: Notifier;
  /**
   * Sets (or, with null or an empty string, removes) the Google Books API key the online
   * lookups of *Add books* and *Suggest genres* use. It is stored encrypted and never sent
   * back (`onlineLookupSettings` only says whether one is set)
   */
  updateOnlineLookupSettings: OnlineLookupSettings;
  updatePublicUrl: ServerConfigModel;
  /**
   * Renames one of the current user's reading lists, or changes its description or
   * visibility.
   *
   * # Returns
   *
   * A result containing the updated reading list, or an error if update failed.
   */
  updateReadingList: ReadingList;
  updateScheduledJob: ScheduledJob;
  updateSeriesMetadata: Series;
  /**
   * Update the thumbnail for a series. This will replace the existing thumbnail with the the one
   * associated with the provided input (book). If the book does not have a thumbnail, one
   * will be generated based on the library's thumbnail configuration.
   */
  updateSeriesThumbnail: Series;
  updateSmartList: SmartList;
  updateSmartListView: SmartListView;
  /** Update the status of a suggestion (Admin+) */
  updateSuggestionStatus: BookClubBookSuggestion;
  updateUser: User;
  updateUserLockStatus: User;
  updateViewer: User;
  updateViewerPreferences: UserPreferences;
  uploadBooks: Scalars['Boolean']['output'];
  /** Upload a new custom emoji */
  uploadCustomEmoji: CustomEmoji;
  uploadLibraryThumbnail: Library;
  uploadMediaThumbnail: Media;
  /**
   * Upload a media thumbnail from a base64-encoded image string.
   * Note: This was added specifically for Komf, which would have been annyoing to
   * implement multipart uploads for
   */
  uploadMediaThumbnailBase64: Media;
  uploadSeries: Scalars['Boolean']['output'];
  uploadSeriesThumbnail: Series;
  /**
   * Upload a series thumbnail from a base64-encoded image string.
   * Note: This was added specifically for Komf, which would have been annyoing to
   * implement multipart uploads for
   */
  uploadSeriesThumbnailBase64: Series;
  /**
   * Upload an avatar image for either the authenticated viewer or for any user if
   * called by a server owner
   */
  uploadUserAvatar: User;
  /** Upserts a reading device for the authenticated user */
  upsertReadingDevice: ReadingDeviceModel;
  /** Upserts a reading journal entry for the authenticated user and the given reading session */
  upsertReadingSessionJournalEntry: JournalEntryModel;
  /** Validate the provided API token by making a test request using a client instance */
  validateProviderConfig: ProviderCredentialVerification;
  /**
   * "Visit" a library, which will upsert a record of the user's last visit to the library.
   * This is used to inform the UI of the last library which was visited by the user
   */
  visitLibrary: Library;
};


export type MutationAcceptAllPendingMatchesArgs = {
  excludeFields?: InputMaybe<Array<MetadataField>>;
  strategy?: InputMaybe<MergeStrategy>;
};


export type MutationAcceptLocalProgressArgs = {
  ancestorSessionId?: InputMaybe<Scalars['Int']['input']>;
  id: Scalars['ID']['input'];
  input: MediaProgressInput;
};


export type MutationAcceptMediaMatchArgs = {
  candidateIndex: Scalars['Int']['input'];
  excludeFields?: InputMaybe<Array<MetadataField>>;
  mediaId: Scalars['ID']['input'];
  overrides?: InputMaybe<Array<MetadataFieldOverride>>;
  strategy?: InputMaybe<MergeStrategy>;
};


export type MutationAcceptSeriesMatchArgs = {
  candidateIndex: Scalars['Int']['input'];
  excludeFields?: InputMaybe<Array<MetadataField>>;
  overrides?: InputMaybe<Array<MetadataFieldOverride>>;
  seriesId: Scalars['ID']['input'];
  strategy?: InputMaybe<MergeStrategy>;
};


export type MutationAddBookToClubArgs = {
  bookClubId: Scalars['ID']['input'];
  input: AddBookToClubInput;
};


export type MutationAddBooksToReadingListArgs = {
  id: Scalars['ID']['input'];
  mediaIds: Array<Scalars['ID']['input']>;
};


export type MutationAnalyzeLibraryArgs = {
  forceReanalysis?: Scalars['Boolean']['input'];
  id: Scalars['ID']['input'];
};


export type MutationAnalyzeMediaArgs = {
  forceReanalysis?: Scalars['Boolean']['input'];
  id: Scalars['ID']['input'];
};


export type MutationAnalyzeSeriesArgs = {
  forceReanalysis?: Scalars['Boolean']['input'];
  id: Scalars['ID']['input'];
};


export type MutationArchiveDiscussionArgs = {
  archived: Scalars['Boolean']['input'];
  discussionId: Scalars['ID']['input'];
};


export type MutationCancelJobArgs = {
  id: Scalars['ID']['input'];
};


export type MutationCleanLibraryArgs = {
  id: Scalars['ID']['input'];
};


export type MutationClearLibraryScannedDirectoriesArgs = {
  id: Scalars['ID']['input'];
};


export type MutationClearMediaProgressArgs = {
  id: Scalars['ID']['input'];
};


export type MutationClearScanHistoryArgs = {
  id: Scalars['ID']['input'];
};


export type MutationClearSeriesReadingHistoryArgs = {
  id: Scalars['ID']['input'];
};


export type MutationCompleteBookArgs = {
  bookClubBookId: Scalars['ID']['input'];
};


export type MutationConvertMediaArgs = {
  id: Scalars['ID']['input'];
};


export type MutationCreateAnnotationArgs = {
  input: CreateAnnotationInput;
};


export type MutationCreateApiKeyArgs = {
  input: ApikeyInput;
};


export type MutationCreateBookClubArgs = {
  input: CreateBookClubInput;
};


export type MutationCreateBookClubInvitationArgs = {
  id: Scalars['ID']['input'];
  input: BookClubInvitationInput;
};


export type MutationCreateBookClubMemberArgs = {
  bookClubId: Scalars['ID']['input'];
  input: CreateBookClubMemberInput;
};


export type MutationCreateBookmarkArgs = {
  input: BookmarkInput;
};


export type MutationCreateDiscussionArgs = {
  bookClubId: Scalars['ID']['input'];
  input: BookClubDiscussionInput;
};


export type MutationCreateEmailDeviceArgs = {
  input: EmailDeviceInput;
};


export type MutationCreateEmailerArgs = {
  input: EmailerInput;
};


export type MutationCreateLibraryArgs = {
  input: CreateOrUpdateLibraryInput;
};


export type MutationCreateMetadataProviderArgs = {
  input: CreateMetadataProviderConfigInput;
};


export type MutationCreateNotifierArgs = {
  input: NotifierInput;
};


export type MutationCreateReadingListArgs = {
  input: ReadingListInput;
};


export type MutationCreateScheduledJobArgs = {
  input: CreateScheduledJobInput;
};


export type MutationCreateSmartListArgs = {
  input: SaveSmartListInput;
};


export type MutationCreateSmartListViewArgs = {
  input: SaveSmartListView;
};


export type MutationCreateTagsArgs = {
  tags: Array<Scalars['String']['input']>;
};


export type MutationCreateUserArgs = {
  input: CreateUserInput;
};


export type MutationDeleteAnnotationArgs = {
  id: Scalars['String']['input'];
};


export type MutationDeleteApiKeyArgs = {
  id: Scalars['Int']['input'];
};


export type MutationDeleteBookClubArgs = {
  id: Scalars['ID']['input'];
};


export type MutationDeleteBookmarkArgs = {
  id: Scalars['String']['input'];
};


export type MutationDeleteCustomEmojiArgs = {
  id: Scalars['ID']['input'];
};


export type MutationDeleteEmailDeviceArgs = {
  id: Scalars['Int']['input'];
};


export type MutationDeleteEmailerArgs = {
  id: Scalars['Int']['input'];
};


export type MutationDeleteHeldFilesArgs = {
  ids: Array<Scalars['Int']['input']>;
};


export type MutationDeleteJobArgs = {
  force?: Scalars['Boolean']['input'];
  id: Scalars['ID']['input'];
};


export type MutationDeleteJobLogsArgs = {
  id: Scalars['ID']['input'];
};


export type MutationDeleteJournalEntryArgs = {
  entryId: Scalars['Int']['input'];
};


export type MutationDeleteLibraryArgs = {
  id: Scalars['ID']['input'];
};


export type MutationDeleteLibraryScanHistoryArgs = {
  id: Scalars['ID']['input'];
};


export type MutationDeleteLibraryThumbnailsArgs = {
  id: Scalars['ID']['input'];
};


export type MutationDeleteLogsArgs = {
  filter?: LogFilterInput;
};


export type MutationDeleteMediaArgs = {
  id: Scalars['ID']['input'];
};


export type MutationDeleteMediaReadingHistoryArgs = {
  id: Scalars['ID']['input'];
};


export type MutationDeleteMessageArgs = {
  messageId: Scalars['ID']['input'];
};


export type MutationDeleteMetadataProviderArgs = {
  id: Scalars['Int']['input'];
};


export type MutationDeleteNotifierArgs = {
  id: Scalars['Int']['input'];
};


export type MutationDeleteReadingDeviceArgs = {
  deviceId: Scalars['Int']['input'];
};


export type MutationDeleteReadingListArgs = {
  id: Scalars['String']['input'];
};


export type MutationDeleteReadingSessionArgs = {
  sessionId: Scalars['Int']['input'];
};


export type MutationDeleteScheduledJobArgs = {
  id: Scalars['Int']['input'];
};


export type MutationDeleteSmartListArgs = {
  id: Scalars['ID']['input'];
};


export type MutationDeleteSmartListViewArgs = {
  id: Scalars['ID']['input'];
  name: Scalars['String']['input'];
};


export type MutationDeleteTagsArgs = {
  tags: Array<Scalars['String']['input']>;
};


export type MutationDeleteUserArgs = {
  hardDelete?: InputMaybe<Scalars['Boolean']['input']>;
  id: Scalars['ID']['input'];
};


export type MutationDeleteUserAvatarArgs = {
  id?: InputMaybe<Scalars['ID']['input']>;
};


export type MutationDeleteUserSessionsArgs = {
  id: Scalars['ID']['input'];
};


export type MutationEditMediaMetadataArgs = {
  id: Scalars['ID']['input'];
  input: MediaMetadataInput;
};


export type MutationEditMediaMetadataBatchArgs = {
  inputs: Array<MediaMetadataBatchInput>;
  options?: InputMaybe<MediaMetadataBatchOptions>;
};


export type MutationEditMessageArgs = {
  input: EditMessageInput;
  messageId: Scalars['ID']['input'];
};


export type MutationEmptyDeletedBooksArgs = {
  libraryId?: InputMaybe<Scalars['ID']['input']>;
};


export type MutationExtendReadingSessionArgs = {
  sessionId: Scalars['Int']['input'];
};


export type MutationFavoriteMediaArgs = {
  id: Scalars['ID']['input'];
  isFavorite: Scalars['Boolean']['input'];
};


export type MutationFavoriteSeriesArgs = {
  id: Scalars['ID']['input'];
  isFavorite: Scalars['Boolean']['input'];
};


export type MutationFetchLibraryMetadataArgs = {
  forceRefetch?: Scalars['Boolean']['input'];
  id: Scalars['ID']['input'];
};


export type MutationFetchMediaMetadataArgs = {
  id: Scalars['ID']['input'];
  search?: InputMaybe<MediaMetadataSearchInput>;
};


export type MutationFetchSeriesMetadataArgs = {
  id: Scalars['ID']['input'];
};


export type MutationFinishMediaProgressArgs = {
  dnf?: InputMaybe<Scalars['Boolean']['input']>;
  id: Scalars['ID']['input'];
};


export type MutationFinishSeriesProgressArgs = {
  id: Scalars['ID']['input'];
};


export type MutationGenerateLibraryThumbnailsArgs = {
  forceRegenerate?: Scalars['Boolean']['input'];
  id: Scalars['ID']['input'];
};


export type MutationIntakeAcceptArgs = {
  ids: Array<Scalars['ID']['input']>;
};


export type MutationIntakeDiscardArgs = {
  ids: Array<Scalars['ID']['input']>;
};


export type MutationIntakeReanalyseArgs = {
  id: Scalars['ID']['input'];
  online?: InputMaybe<Scalars['Boolean']['input']>;
};


export type MutationIntakeReviewArgs = {
  id: Scalars['ID']['input'];
  input: IntakeReviewInput;
};


export type MutationIntakeUploadArgs = {
  files: Array<Scalars['Upload']['input']>;
  libraryId: Scalars['ID']['input'];
  mode?: IntakeUploadMode;
  relativePaths?: InputMaybe<Array<Scalars['String']['input']>>;
};


export type MutationLeaveBookClubArgs = {
  bookClubId: Scalars['ID']['input'];
};


export type MutationLockDiscussionArgs = {
  discussionId: Scalars['ID']['input'];
  locked: Scalars['Boolean']['input'];
};


export type MutationPatchEmailDeviceArgs = {
  id: Scalars['Int']['input'];
  input: PatchEmailDeviceInput;
};


export type MutationPatchLibraryArgs = {
  id: Scalars['ID']['input'];
  input: PatchLibraryInput;
};


export type MutationPatchLibraryConfigArgs = {
  id: Scalars['ID']['input'];
  input: PatchLibraryConfigInput;
};


export type MutationPatchReadingSessionArgs = {
  input: PatchReadingSession;
  sessionId: Scalars['Int']['input'];
};


export type MutationPinMessageArgs = {
  messageId: Scalars['ID']['input'];
  pinned: Scalars['Boolean']['input'];
};


export type MutationProcessLibraryThumbnailsArgs = {
  forceRegenerate?: Scalars['Boolean']['input'];
  id: Scalars['ID']['input'];
};


export type MutationPurgeDeletedBooksArgs = {
  ids: Array<Scalars['ID']['input']>;
};


export type MutationPutBackHeldFileArgs = {
  id: Scalars['Int']['input'];
};


export type MutationRejectMediaMatchArgs = {
  candidateIndex: Scalars['Int']['input'];
  mediaId: Scalars['ID']['input'];
};


export type MutationRejectSeriesMatchArgs = {
  candidateIndex: Scalars['Int']['input'];
  seriesId: Scalars['ID']['input'];
};


export type MutationRemoveBookClubMemberArgs = {
  bookClubId: Scalars['ID']['input'];
  memberId: Scalars['ID']['input'];
};


export type MutationRemoveBooksFromReadingListArgs = {
  id: Scalars['ID']['input'];
  mediaIds: Array<Scalars['ID']['input']>;
};


export type MutationRemoveSuggestionArgs = {
  suggestionId: Scalars['ID']['input'];
};


export type MutationRenameTagArgs = {
  id: Scalars['Int']['input'];
  name: Scalars['String']['input'];
};


export type MutationReorderBooksArgs = {
  bookClubId: Scalars['ID']['input'];
  bookIds: Array<Scalars['String']['input']>;
};


export type MutationResetElapsedSecondsArgs = {
  id: Scalars['ID']['input'];
};


export type MutationResetLibraryMetadataArgs = {
  id: Scalars['ID']['input'];
  impact: MetadataResetImpact;
};


export type MutationResetSeriesMetadataArgs = {
  id: Scalars['ID']['input'];
  impact: MetadataResetImpact;
};


export type MutationRespondToBookClubInvitationArgs = {
  id: Scalars['ID']['input'];
  input: BookClubInvitationResponseInput;
};


export type MutationRestoreDeletedBooksArgs = {
  ids: Array<Scalars['ID']['input']>;
};


export type MutationRestoreMediaArgs = {
  id: Scalars['ID']['input'];
};


export type MutationRevertMediaMetadataEditArgs = {
  mediaId: Scalars['ID']['input'];
  saveId: Scalars['String']['input'];
};


export type MutationRunPostProcessingArgs = {
  dryRun?: Scalars['Boolean']['input'];
  grey16?: Scalars['Boolean']['input'];
  libraryId: Scalars['ID']['input'];
  pdfOcr?: Scalars['Boolean']['input'];
  pdfShrink?: Scalars['Boolean']['input'];
};


export type MutationScanLibraryArgs = {
  id: Scalars['ID']['input'];
  options?: InputMaybe<Scalars['JSON']['input']>;
};


export type MutationScanSeriesArgs = {
  id: Scalars['ID']['input'];
};


export type MutationSendAttachmentEmailArgs = {
  input: SendAttachmentEmailsInput;
};


export type MutationSendMessageArgs = {
  discussionId: Scalars['ID']['input'];
  input: SendMessageInput;
};


export type MutationSetLibraryMediaLockedFieldsArgs = {
  libraryId: Scalars['ID']['input'];
  lockedFields: Array<MetadataField>;
};


export type MutationSetLibrarySeriesLockedFieldsArgs = {
  libraryId: Scalars['ID']['input'];
  lockedFields: Array<MetadataField>;
};


export type MutationSetMediaLockedFieldsArgs = {
  lockedFields: Array<MetadataField>;
  mediaId: Scalars['ID']['input'];
};


export type MutationSetMediaTagsArgs = {
  id: Scalars['ID']['input'];
  tags: Array<Scalars['String']['input']>;
};


export type MutationSetSeriesLockedFieldsArgs = {
  lockedFields: Array<MetadataField>;
  seriesId: Scalars['ID']['input'];
};


export type MutationSetSeriesTagsArgs = {
  id: Scalars['ID']['input'];
  tags: Array<Scalars['String']['input']>;
};


export type MutationSuggestBookArgs = {
  bookClubId: Scalars['ID']['input'];
  input: SuggestBookInput;
};


export type MutationTestEmailerArgs = {
  config: EmailerClientConfig;
  recipient: Scalars['String']['input'];
};


export type MutationToggleReactionArgs = {
  customEmojiId?: InputMaybe<Scalars['Int']['input']>;
  emoji?: InputMaybe<Scalars['String']['input']>;
  messageId: Scalars['ID']['input'];
};


export type MutationToggleSuggestionLikeArgs = {
  suggestionId: Scalars['ID']['input'];
};


export type MutationUpdateAnnotationArgs = {
  input: UpdateAnnotationInput;
};


export type MutationUpdateApiKeyArgs = {
  id: Scalars['Int']['input'];
  input: ApikeyInput;
};


export type MutationUpdateBookClubArgs = {
  id: Scalars['ID']['input'];
  input: UpdateBookClubInput;
};


export type MutationUpdateCustomEmojiArgs = {
  id: Scalars['ID']['input'];
  input: UpdateCustomEmojiInput;
};


export type MutationUpdateDeletedBooksRetentionArgs = {
  days: Scalars['Int']['input'];
};


export type MutationUpdateEmailDeviceArgs = {
  id: Scalars['Int']['input'];
  input: EmailDeviceInput;
};


export type MutationUpdateEmailerArgs = {
  id: Scalars['Int']['input'];
  input: EmailerInput;
};


export type MutationUpdateHomeArrangementArgs = {
  input: HomeArrangementInput;
};


export type MutationUpdateLibraryArgs = {
  id: Scalars['ID']['input'];
  input: CreateOrUpdateLibraryInput;
};


export type MutationUpdateLibraryEmojiArgs = {
  emoji?: InputMaybe<Scalars['String']['input']>;
  id: Scalars['ID']['input'];
};


export type MutationUpdateLibraryExcludedUsersArgs = {
  id: Scalars['ID']['input'];
  userIds: Array<Scalars['String']['input']>;
};


export type MutationUpdateLibraryIntakeSettingsArgs = {
  input: LibraryIntakeSettingsInput;
  libraryId: Scalars['ID']['input'];
};


export type MutationUpdateLibraryPostProcessingArgs = {
  input: PostProcessingConfigInput;
  libraryId: Scalars['ID']['input'];
};


export type MutationUpdateLibraryThumbnailArgs = {
  id: Scalars['ID']['input'];
  input: UpdateThumbnailInput;
};


export type MutationUpdateMediaMetadataArgs = {
  id: Scalars['ID']['input'];
  input: MediaMetadataInput;
};


export type MutationUpdateMediaProgressArgs = {
  id: Scalars['ID']['input'];
  input: MediaProgressInput;
};


export type MutationUpdateMediaThumbnailArgs = {
  id: Scalars['ID']['input'];
  input: PageBasedThumbnailInput;
};


export type MutationUpdateMetadataFetchSettingsArgs = {
  fetchAfterScan: Scalars['Boolean']['input'];
};


export type MutationUpdateMetadataProviderArgs = {
  id: Scalars['Int']['input'];
  input: PatchMetadataProviderConfigInput;
};


export type MutationUpdateMyReaderSettingsArgs = {
  settings: Scalars['String']['input'];
};


export type MutationUpdateNavigationArrangementArgs = {
  input: NavigationArrangementInput;
};


export type MutationUpdateNavigationArrangementLockArgs = {
  locked: Scalars['Boolean']['input'];
};


export type MutationUpdateNotifierArgs = {
  id: Scalars['Int']['input'];
  input: NotifierInput;
};


export type MutationUpdateOnlineLookupSettingsArgs = {
  googleBooksApiKey?: InputMaybe<Scalars['String']['input']>;
};


export type MutationUpdatePublicUrlArgs = {
  publicUrl: Scalars['String']['input'];
};


export type MutationUpdateReadingListArgs = {
  id: Scalars['ID']['input'];
  input: UpdateReadingListInput;
};


export type MutationUpdateScheduledJobArgs = {
  id: Scalars['Int']['input'];
  input: UpdateScheduledJobInput;
};


export type MutationUpdateSeriesMetadataArgs = {
  id: Scalars['ID']['input'];
  input: SeriesMetadataInput;
};


export type MutationUpdateSeriesThumbnailArgs = {
  id: Scalars['ID']['input'];
  input: UpdateThumbnailInput;
};


export type MutationUpdateSmartListArgs = {
  id: Scalars['ID']['input'];
  input: SaveSmartListInput;
};


export type MutationUpdateSmartListViewArgs = {
  input: SaveSmartListView;
  originalName: Scalars['String']['input'];
};


export type MutationUpdateSuggestionStatusArgs = {
  notes?: InputMaybe<Scalars['String']['input']>;
  status: BookClubSuggestionStatus;
  suggestionId: Scalars['ID']['input'];
};


export type MutationUpdateUserArgs = {
  id: Scalars['ID']['input'];
  input: UpdateUserInput;
};


export type MutationUpdateUserLockStatusArgs = {
  id: Scalars['ID']['input'];
  lock: Scalars['Boolean']['input'];
};


export type MutationUpdateViewerArgs = {
  input: UpdateUserInput;
};


export type MutationUpdateViewerPreferencesArgs = {
  input: UpdateUserPreferencesInput;
};


export type MutationUploadBooksArgs = {
  input: UploadBooksInput;
};


export type MutationUploadCustomEmojiArgs = {
  input: CreateCustomEmojiInput;
  upload: Scalars['Upload']['input'];
};


export type MutationUploadLibraryThumbnailArgs = {
  file: Scalars['Upload']['input'];
  id: Scalars['ID']['input'];
};


export type MutationUploadMediaThumbnailArgs = {
  file: Scalars['Upload']['input'];
  id: Scalars['ID']['input'];
};


export type MutationUploadMediaThumbnailBase64Args = {
  id: Scalars['ID']['input'];
  image: Scalars['String']['input'];
};


export type MutationUploadSeriesArgs = {
  input: UploadSeriesInput;
};


export type MutationUploadSeriesThumbnailArgs = {
  file: Scalars['Upload']['input'];
  id: Scalars['ID']['input'];
};


export type MutationUploadSeriesThumbnailBase64Args = {
  id: Scalars['ID']['input'];
  image: Scalars['String']['input'];
};


export type MutationUploadUserAvatarArgs = {
  id?: InputMaybe<Scalars['ID']['input']>;
  upload: Scalars['Upload']['input'];
};


export type MutationUpsertReadingDeviceArgs = {
  id: Scalars['String']['input'];
  name: Scalars['String']['input'];
};


export type MutationUpsertReadingSessionJournalEntryArgs = {
  content: Scalars['String']['input'];
  sessionId: Scalars['Int']['input'];
};


export type MutationValidateProviderConfigArgs = {
  config: ValidateMetadataProviderConfigInput;
};


export type MutationVisitLibraryArgs = {
  id: Scalars['ID']['input'];
};

export type NavigationArrangementInput = {
  sections: Array<ArrangementSectionInput>;
};

export type Notifier = {
  __typename?: 'Notifier';
  config: NotifierConfig;
  id: Scalars['Int']['output'];
  type: Scalars['String']['output'];
};

export type NotifierConfig = DiscordConfig | TelegramConfig;

export type NotifierInput =
  { discord: DiscordConfigInput; telegram?: never; }
  |  { discord?: never; telegram: TelegramConfigInput; };

export type NumericFilterDateTime =
  { anyOf: Array<Scalars['DateTime']['input']>; eq?: never; gt?: never; gte?: never; lt?: never; lte?: never; neq?: never; noneOf?: never; range?: never; }
  |  { anyOf?: never; eq: Scalars['DateTime']['input']; gt?: never; gte?: never; lt?: never; lte?: never; neq?: never; noneOf?: never; range?: never; }
  |  { anyOf?: never; eq?: never; gt: Scalars['DateTime']['input']; gte?: never; lt?: never; lte?: never; neq?: never; noneOf?: never; range?: never; }
  |  { anyOf?: never; eq?: never; gt?: never; gte: Scalars['DateTime']['input']; lt?: never; lte?: never; neq?: never; noneOf?: never; range?: never; }
  |  { anyOf?: never; eq?: never; gt?: never; gte?: never; lt: Scalars['DateTime']['input']; lte?: never; neq?: never; noneOf?: never; range?: never; }
  |  { anyOf?: never; eq?: never; gt?: never; gte?: never; lt?: never; lte: Scalars['DateTime']['input']; neq?: never; noneOf?: never; range?: never; }
  |  { anyOf?: never; eq?: never; gt?: never; gte?: never; lt?: never; lte?: never; neq: Scalars['DateTime']['input']; noneOf?: never; range?: never; }
  |  { anyOf?: never; eq?: never; gt?: never; gte?: never; lt?: never; lte?: never; neq?: never; noneOf: Array<Scalars['DateTime']['input']>; range?: never; }
  |  { anyOf?: never; eq?: never; gt?: never; gte?: never; lt?: never; lte?: never; neq?: never; noneOf?: never; range: NumericRangeDateTime; };

export type NumericFilterF32 =
  { anyOf: Array<Scalars['Float']['input']>; eq?: never; gt?: never; gte?: never; lt?: never; lte?: never; neq?: never; noneOf?: never; range?: never; }
  |  { anyOf?: never; eq: Scalars['Float']['input']; gt?: never; gte?: never; lt?: never; lte?: never; neq?: never; noneOf?: never; range?: never; }
  |  { anyOf?: never; eq?: never; gt: Scalars['Float']['input']; gte?: never; lt?: never; lte?: never; neq?: never; noneOf?: never; range?: never; }
  |  { anyOf?: never; eq?: never; gt?: never; gte: Scalars['Float']['input']; lt?: never; lte?: never; neq?: never; noneOf?: never; range?: never; }
  |  { anyOf?: never; eq?: never; gt?: never; gte?: never; lt: Scalars['Float']['input']; lte?: never; neq?: never; noneOf?: never; range?: never; }
  |  { anyOf?: never; eq?: never; gt?: never; gte?: never; lt?: never; lte: Scalars['Float']['input']; neq?: never; noneOf?: never; range?: never; }
  |  { anyOf?: never; eq?: never; gt?: never; gte?: never; lt?: never; lte?: never; neq: Scalars['Float']['input']; noneOf?: never; range?: never; }
  |  { anyOf?: never; eq?: never; gt?: never; gte?: never; lt?: never; lte?: never; neq?: never; noneOf: Array<Scalars['Float']['input']>; range?: never; }
  |  { anyOf?: never; eq?: never; gt?: never; gte?: never; lt?: never; lte?: never; neq?: never; noneOf?: never; range: NumericRangeF32; };

export type NumericFilterI32 =
  { anyOf: Array<Scalars['Int']['input']>; eq?: never; gt?: never; gte?: never; lt?: never; lte?: never; neq?: never; noneOf?: never; range?: never; }
  |  { anyOf?: never; eq: Scalars['Int']['input']; gt?: never; gte?: never; lt?: never; lte?: never; neq?: never; noneOf?: never; range?: never; }
  |  { anyOf?: never; eq?: never; gt: Scalars['Int']['input']; gte?: never; lt?: never; lte?: never; neq?: never; noneOf?: never; range?: never; }
  |  { anyOf?: never; eq?: never; gt?: never; gte: Scalars['Int']['input']; lt?: never; lte?: never; neq?: never; noneOf?: never; range?: never; }
  |  { anyOf?: never; eq?: never; gt?: never; gte?: never; lt: Scalars['Int']['input']; lte?: never; neq?: never; noneOf?: never; range?: never; }
  |  { anyOf?: never; eq?: never; gt?: never; gte?: never; lt?: never; lte: Scalars['Int']['input']; neq?: never; noneOf?: never; range?: never; }
  |  { anyOf?: never; eq?: never; gt?: never; gte?: never; lt?: never; lte?: never; neq: Scalars['Int']['input']; noneOf?: never; range?: never; }
  |  { anyOf?: never; eq?: never; gt?: never; gte?: never; lt?: never; lte?: never; neq?: never; noneOf: Array<Scalars['Int']['input']>; range?: never; }
  |  { anyOf?: never; eq?: never; gt?: never; gte?: never; lt?: never; lte?: never; neq?: never; noneOf?: never; range: NumericRangeI32; };

export type NumericFilterI64 =
  { anyOf: Array<Scalars['Int']['input']>; eq?: never; gt?: never; gte?: never; lt?: never; lte?: never; neq?: never; noneOf?: never; range?: never; }
  |  { anyOf?: never; eq: Scalars['Int']['input']; gt?: never; gte?: never; lt?: never; lte?: never; neq?: never; noneOf?: never; range?: never; }
  |  { anyOf?: never; eq?: never; gt: Scalars['Int']['input']; gte?: never; lt?: never; lte?: never; neq?: never; noneOf?: never; range?: never; }
  |  { anyOf?: never; eq?: never; gt?: never; gte: Scalars['Int']['input']; lt?: never; lte?: never; neq?: never; noneOf?: never; range?: never; }
  |  { anyOf?: never; eq?: never; gt?: never; gte?: never; lt: Scalars['Int']['input']; lte?: never; neq?: never; noneOf?: never; range?: never; }
  |  { anyOf?: never; eq?: never; gt?: never; gte?: never; lt?: never; lte: Scalars['Int']['input']; neq?: never; noneOf?: never; range?: never; }
  |  { anyOf?: never; eq?: never; gt?: never; gte?: never; lt?: never; lte?: never; neq: Scalars['Int']['input']; noneOf?: never; range?: never; }
  |  { anyOf?: never; eq?: never; gt?: never; gte?: never; lt?: never; lte?: never; neq?: never; noneOf: Array<Scalars['Int']['input']>; range?: never; }
  |  { anyOf?: never; eq?: never; gt?: never; gte?: never; lt?: never; lte?: never; neq?: never; noneOf?: never; range: NumericRangeI64; };

export type NumericRangeDateTime = {
  from: Scalars['DateTime']['input'];
  inclusive: Scalars['Boolean']['input'];
  to: Scalars['DateTime']['input'];
};

export type NumericRangeF32 = {
  from: Scalars['Float']['input'];
  inclusive: Scalars['Boolean']['input'];
  to: Scalars['Float']['input'];
};

export type NumericRangeI32 = {
  from: Scalars['Int']['input'];
  inclusive: Scalars['Boolean']['input'];
  to: Scalars['Int']['input'];
};

export type NumericRangeI64 = {
  from: Scalars['Int']['input'];
  inclusive: Scalars['Boolean']['input'];
  to: Scalars['Int']['input'];
};

/** A simple offset-based pagination input object */
export type OffsetPagination = {
  /**
   * The page to start from. This is 1-based by default, but can be
   * changed to 0-based by setting the `zero_based` field to true.
   */
  page: Scalars['Int']['input'];
  /** The number of items to return per page. This is 20 by default. */
  pageSize?: InputMaybe<Scalars['Int']['input']>;
  /** Whether or not the page is zero-based. This is false by default. */
  zeroBased?: InputMaybe<Scalars['Boolean']['input']>;
};

/** Information about the current offset pagination state */
export type OffsetPaginationInfo = {
  __typename?: 'OffsetPaginationInfo';
  /** The current page, zero-indexed. */
  currentPage: Scalars['Int']['output'];
  /** The offset of the current page. E.g. if current page is 1, and pageSize is 10, the offset is 20. */
  pageOffset: Scalars['Int']['output'];
  /** The number of elements per page. */
  pageSize: Scalars['Int']['output'];
  /** The total number of items available */
  totalItems: Scalars['Int']['output'];
  /**
   * The number of pages available. This is **not** affected by the zero-based flag,
   * so a client requesting zero-based pagination will need to adjust their pagination
   * logic accordingly.
   */
  totalPages: Scalars['Int']['output'];
  /** Whether or not the page is zero-indexed. */
  zeroBased: Scalars['Boolean']['output'];
};

export type OnDeckBooks = {
  __typename?: 'OnDeckBooks';
  name?: Maybe<Scalars['String']['output']>;
};

export type OnDeckBooksInput = {
  name?: InputMaybe<Scalars['String']['input']>;
};

/** One online source's answer to a lookup, for the review screens */
export type OnlineLookup = {
  __typename?: 'OnlineLookup';
  /**
   * What happened, in a few words (e.g. "daily limit reached: add an API key in
   * Settings")
   */
  message: Scalars['String']['output'];
  /** `OPEN_LIBRARY`, `GOOGLE_BOOKS` or `HARDCOVER` */
  source: IntakeSource;
  status: OnlineLookupStatus;
  /** How many subjects it gave */
  subjects: Scalars['Int']['output'];
};

/** The online lookups of *Add books* and *Suggest genres* (Open Library and Google Books) */
export type OnlineLookupSettings = {
  __typename?: 'OnlineLookupSettings';
  /**
   * Whether a Google Books API key is set (the key itself is never sent back). Without one,
   * Google Books lookups share Google's anonymous daily quota, which is often used up
   */
  googleBooksApiKeySet: Scalars['Boolean']['output'];
};

/** How an online source answered a lookup */
export enum OnlineLookupStatus {
  /** The source failed (network, timeout, an unexpected answer, a refused API key) */
  Error = 'ERROR',
  /** The source isn't set up: Hardcover's metadata provider is off or has no API token */
  NotConfigured = 'NOT_CONFIGURED',
  /** The source answered, but has no matching book */
  NoMatch = 'NO_MATCH',
  /** The book was found, but the source has no subjects for it */
  NoSubjects = 'NO_SUBJECTS',
  /** The book was found, with subjects */
  Ok = 'OK',
  /**
   * The source refused for now: its request quota (Google Books' shared daily quota
   * without an API key) is used up
   */
  RateLimited = 'RATE_LIMITED'
}

export enum OrderDirection {
  Asc = 'ASC',
  Desc = 'DESC'
}

export enum OrganizationMode {
  Filesystem = 'FILESYSTEM',
  Metadata = 'METADATA'
}

export type PageBasedThumbnailInput = {
  /** A flag indicating whether the page is zero based (i.e. 0 is the first page) */
  isZeroBased?: InputMaybe<Scalars['Boolean']['input']>;
  /** The page to pull inside the media file for generating the thumbnail */
  page: Scalars['Int']['input'];
};

/**
 * Represents a page dimension for a page of a Stump media item. It consists of a
 * height and a width.
 */
export type PageDimension = {
  __typename?: 'PageDimension';
  height: Scalars['Int']['output'];
  width: Scalars['Int']['output'];
};

export type PagedProgressInput = {
  deviceId?: InputMaybe<Scalars['String']['input']>;
  elapsedSecondsDelta?: InputMaybe<Scalars['Int']['input']>;
  page: Scalars['Int']['input'];
  resetElapsedSeconds?: InputMaybe<Scalars['Boolean']['input']>;
};

export type PaginatedAuthorResponse = {
  __typename?: 'PaginatedAuthorResponse';
  nodes: Array<Author>;
  pageInfo: PaginationInfo;
};

export type PaginatedDirectoryListingResponse = {
  __typename?: 'PaginatedDirectoryListingResponse';
  nodes: Array<DirectoryListing>;
  pageInfo: PaginationInfo;
};

export type PaginatedJobResponse = {
  __typename?: 'PaginatedJobResponse';
  nodes: Array<Job>;
  pageInfo: PaginationInfo;
};

export type PaginatedLibraryResponse = {
  __typename?: 'PaginatedLibraryResponse';
  nodes: Array<Library>;
  pageInfo: PaginationInfo;
};

export type PaginatedLogResponse = {
  __typename?: 'PaginatedLogResponse';
  nodes: Array<Log>;
  pageInfo: PaginationInfo;
};

export type PaginatedMediaResponse = {
  __typename?: 'PaginatedMediaResponse';
  nodes: Array<Media>;
  pageInfo: PaginationInfo;
};

export type PaginatedMissingEntityResponse = {
  __typename?: 'PaginatedMissingEntityResponse';
  nodes: Array<MissingEntity>;
  pageInfo: PaginationInfo;
};

export type PaginatedReadingListResponse = {
  __typename?: 'PaginatedReadingListResponse';
  nodes: Array<ReadingList>;
  pageInfo: PaginationInfo;
};

export type PaginatedSeriesResponse = {
  __typename?: 'PaginatedSeriesResponse';
  nodes: Array<Series>;
  pageInfo: PaginationInfo;
};

export type PaginatedUserResponse = {
  __typename?: 'PaginatedUserResponse';
  nodes: Array<User>;
  pageInfo: PaginationInfo;
};

/**
 * A union of the supported pagination flavors which Stump supports. The resulting
 * response will be dependent on the pagination type used, e.g. a [CursorPaginatedResponse]
 * will be returned if the [CursorPagination] type is used.
 *
 * You may use a conditional fragment in your GraphQL query for type-specific fields:
 * ```graphql
 * query MyQuery {
 * media(pagination: { offset: { page: 1, pageSize: 20 } }) {
 * ... on OffsetPaginationInfo {
 * totalPages
 * currentPage
 * }
 * }
 * }
 * ```
 *
 * A special case is the `None` variant, which will return an offset-based pagination info
 * object based on the size of the result set. This will not paginate the results, so be
 * cautious when using this with large result sets.
 *
 * **Note**: Be sure to call [Pagination::resolve] before using the pagination object
 * to ensure that the pagination object is in a valid state.
 */
export type Pagination =
  { cursor: CursorPagination; none?: never; offset?: never; }
  |  { cursor?: never; none: Unpaginated; offset?: never; }
  |  { cursor?: never; none?: never; offset: OffsetPagination; };

export type PaginationInfo = CursorPaginationInfo | OffsetPaginationInfo;

export type PatchEmailDeviceInput = {
  email?: InputMaybe<Scalars['String']['input']>;
  forbidden?: InputMaybe<Scalars['Boolean']['input']>;
  name?: InputMaybe<Scalars['String']['input']>;
};

export type PatchLibraryConfigInput = {
  convertRarToZip?: InputMaybe<Scalars['Boolean']['input']>;
  defaultLibraryViewMode?: InputMaybe<LibraryViewMode>;
  defaultReadingDir?: InputMaybe<ReadingDirection>;
  defaultReadingImageScaleFit?: InputMaybe<ReadingImageScaleFit>;
  defaultReadingMode?: InputMaybe<ReadingMode>;
  generateFileHashes?: InputMaybe<Scalars['Boolean']['input']>;
  generateKoreaderHashes?: InputMaybe<Scalars['Boolean']['input']>;
  hardDeleteConversions?: InputMaybe<Scalars['Boolean']['input']>;
  hideSeriesView?: InputMaybe<Scalars['Boolean']['input']>;
  ignoreRules?: InputMaybe<Array<Scalars['String']['input']>>;
  libraryPattern?: InputMaybe<LibraryPattern>;
  libraryType?: InputMaybe<LibraryType>;
  oneshotsDirectory?: InputMaybe<Scalars['String']['input']>;
  processMetadata?: InputMaybe<Scalars['Boolean']['input']>;
  processThumbnailColorsEvenWithoutConfig?: InputMaybe<Scalars['Boolean']['input']>;
  skipBookOverview?: InputMaybe<Scalars['Boolean']['input']>;
  thumbnailConfig?: InputMaybe<ImageProcessorOptionsInput>;
  watch?: InputMaybe<Scalars['Boolean']['input']>;
};

export type PatchLibraryInput = {
  config?: InputMaybe<PatchLibraryConfigInput>;
  description?: InputMaybe<Scalars['String']['input']>;
  emoji?: InputMaybe<Scalars['String']['input']>;
  name?: InputMaybe<Scalars['String']['input']>;
  path?: InputMaybe<Scalars['String']['input']>;
  scanAfterPersist?: Scalars['Boolean']['input'];
  tags?: InputMaybe<Array<Scalars['String']['input']>>;
};

/** A patch equivalent of [CreateMetadataProviderConfigInput], i.e. just with optional fields. */
export type PatchMetadataProviderConfigInput = {
  /** The API token for authenticating with the provider */
  apiToken?: InputMaybe<Scalars['String']['input']>;
  /**
   * Optional expiration date for the API key. This is exclusively a QOL thing,
   * since the creds don't live within the management domain of Stump
   */
  apiTokenExpiresAt?: InputMaybe<Scalars['DateTime']['input']>;
  /** Auto-apply configuration */
  autoApplyConfig?: InputMaybe<Scalars['JSON']['input']>;
  /** Whether the provider is enabled */
  enabled?: InputMaybe<Scalars['Boolean']['input']>;
};

export type PatchReadingSession = {
  elapsedSeconds?: InputMaybe<Scalars['Int']['input']>;
  status?: InputMaybe<ReadingStatus>;
};

/** The PDF worker container (OCR and size reduction of scanned PDFs) */
export type PdfWorker = {
  __typename?: 'PdfWorker';
  /** Its heartbeat is recent: the container is running */
  alive: Scalars['Boolean']['output'];
  /** The task ids it is working on */
  current?: Maybe<Scalars['String']['output']>;
  /** When its heartbeat was last refreshed */
  lastSeen?: Maybe<Scalars['DateTime']['output']>;
  /** `idle` or `busy` */
  state?: Maybe<Scalars['String']['output']>;
  /** The worker image's revision */
  version?: Maybe<Scalars['String']['output']>;
};

export type PlaceholderGenerationOutput = {
  __typename?: 'PlaceholderGenerationOutput';
  /** The number of placeholder metadata entries that were generated */
  generatedMetadata: Scalars['Int']['output'];
  /** The number of entities that were skipped (already had metadata or no thumbnail file) */
  skippedEntities: Scalars['Int']['output'];
  /** The total number of entities that were visited */
  visitedEntities: Scalars['Int']['output'];
};

/** What a post-processing task does */
export enum PostProcessKind {
  /** CBZ pages reduced to 16 grey levels (in Stump) */
  Grey_16 = 'GREY_16',
  /** A PDF sent to the PDF worker for OCR and/or shrinking (see `steps`) */
  Pdf = 'PDF'
}

/** When a post-processing step runs for a library */
export enum PostProcessMode {
  /** The step runs on books a scan adds after it was turned on */
  New = 'NEW',
  /** The step never runs on its own (it can still be started from the library settings) */
  Off = 'OFF'
}

export type PostProcessOutput = {
  __typename?: 'PostProcessOutput';
  bytesAfter: Scalars['Int']['output'];
  /** Sizes before and after (or expected after, for a dry run) of the processed volumes */
  bytesBefore: Scalars['Int']['output'];
  /** Volumes whose 16-grey copy replaced the original */
  converted: Scalars['Int']['output'];
  /** Volumes estimated by a dry run */
  estimated: Scalars['Int']['output'];
  failed: Scalars['Int']['output'];
  /** PDFs written to the PDF worker's queue */
  pdfsQueued: Scalars['Int']['output'];
  /** Volumes processed but kept (the saving was too small) */
  rejected: Scalars['Int']['output'];
  /** Volumes with nothing to do (already 16 greys, no grey PNG pages) */
  skipped: Scalars['Int']['output'];
  /** CBZ volumes looked at */
  volumes: Scalars['Int']['output'];
};

/** Where a post-processing task stands */
export enum PostProcessStatus {
  /**
   * The smaller (or OCRed) file replaced the original, which is held in *Deleted books*
   * for the retention (it can be put back from there)
   */
  Done = 'DONE',
  /** A dry run's estimate (nothing was changed) */
  Estimated = 'ESTIMATED',
  /** Something went wrong; the original is untouched */
  Failed = 'FAILED',
  /** Waiting in the queue (PDFs: written for the PDF worker) */
  Pending = 'PENDING',
  /** Processed, but the result was not used (saves too little, or a quality check failed) */
  Rejected = 'REJECTED',
  /** The original was put back from *Deleted books*: the book is left as it is */
  Reverted = 'REVERTED',
  /** Being processed */
  Running = 'RUNNING',
  /** Nothing to do (already 16 greys, born-digital PDF, ...) */
  Skipped = 'SKIPPED'
}

/** A library's post-processing settings (stored as JSON in `library_configs`) */
export type PostProcessingConfig = {
  __typename?: 'PostProcessingConfig';
  /** Reduce the grey pages of CBZ files to 16 grey levels (4-bit PNG); colour pages stay */
  grey16: PostProcessMode;
  /**
   * Parts of a book's path (relative to the library, e.g. `/Berserk/`) that keep a CBZ out
   * of the 16-grey step
   */
  grey16Exclude: Array<Scalars['String']['output']>;
  /** When the 16-grey step was set to new books: books added from then on are processed */
  grey16Since?: Maybe<Scalars['DateTime']['output']>;
  /** OCR scanned PDFs whose text layer is missing or poor (PDF worker container) */
  pdfOcr: PostProcessMode;
  /** When PDF OCR was set to new books */
  pdfOcrSince?: Maybe<Scalars['DateTime']['output']>;
  /** Shrink scanned PDFs when it saves enough without visible loss (PDF worker container) */
  pdfShrink: PostProcessMode;
  /** When PDF shrinking was set to new books */
  pdfShrinkSince?: Maybe<Scalars['DateTime']['output']>;
};

/** The settable part of [`PostProcessingConfig`] */
export type PostProcessingConfigInput = {
  grey16: PostProcessMode;
  grey16Exclude: Array<Scalars['String']['input']>;
  pdfOcr: PostProcessMode;
  pdfShrink: PostProcessMode;
};

export type PostProcessingCount = {
  __typename?: 'PostProcessingCount';
  count: Scalars['Int']['output'];
  kind: PostProcessKind;
  status: PostProcessStatus;
};

/**
 * One post-processing step for one book: the 16-grey reduction of a CBZ, or a PDF sent to
 * the PDF worker for OCR and/or shrinking. There is at most one row per book and kind; a
 * new run updates it in place
 */
export type PostProcessingTaskModel = {
  __typename?: 'PostProcessingTaskModel';
  /** The size of the processed file (for an estimate, the expected size) */
  bytesAfter?: Maybe<Scalars['Int']['output']>;
  bytesBefore?: Maybe<Scalars['Int']['output']>;
  createdAt: Scalars['DateTime']['output'];
  /** Step details (pages converted, quality numbers, ...) */
  details?: Maybe<Scalars['JSON']['output']>;
  finishedAt?: Maybe<Scalars['DateTime']['output']>;
  id: Scalars['String']['output'];
  kind: PostProcessKind;
  libraryId: Scalars['String']['output'];
  mediaId?: Maybe<Scalars['String']['output']>;
  /** The book's path when the task was created */
  path: Scalars['String']['output'];
  /** Why the task ended the way it did, in a few words */
  reason?: Maybe<Scalars['String']['output']>;
  status: PostProcessStatus;
  /** For PDFs, the requested steps (`OCR`, `SHRINK` or `OCR+SHRINK`) */
  steps?: Maybe<Scalars['String']['output']>;
  updatedAt: Scalars['DateTime']['output'];
};

export type ProviderCredentialVerification = {
  __typename?: 'ProviderCredentialVerification';
  error?: Maybe<Scalars['String']['output']>;
  isValid: Scalars['Boolean']['output'];
  responseStatus: Scalars['Int']['output'];
};

export enum PublicationStatus {
  Cancelled = 'CANCELLED',
  Completed = 'COMPLETED',
  Hiatus = 'HIATUS',
  Ongoing = 'ONGOING',
  Upcoming = 'UPCOMING'
}

/**
 * A book deleted permanently: its file and its data were deleted; the row records what it
 * was, where and who deleted it
 */
export type PurgedBook = {
  __typename?: 'PurgedBook';
  authors: Array<Scalars['String']['output']>;
  /** Deleted by the automatic purge */
  automatic: Scalars['Boolean']['output'];
  /** The size of its file (null: it was already gone) */
  bytes?: Maybe<Scalars['Int']['output']>;
  deletedAt?: Maybe<Scalars['DateTime']['output']>;
  deletedBy?: Maybe<Scalars['String']['output']>;
  id: Scalars['Int']['output'];
  libraryId?: Maybe<Scalars['String']['output']>;
  libraryName?: Maybe<Scalars['String']['output']>;
  mediaId: Scalars['String']['output'];
  originalPath: Scalars['String']['output'];
  purgedAt: Scalars['DateTime']['output'];
  /** Who deleted it permanently, when known */
  purgedBy?: Maybe<Scalars['String']['output']>;
  title: Scalars['String']['output'];
};

export type Query = {
  __typename?: 'Query';
  activeReadingSessionCount: Scalars['Int']['output'];
  /** Get all annotations (highlights/notes) for a single book */
  annotationsByMediaId: Array<MediaAnnotation>;
  apiKeyById: Apikey;
  apiKeys: Array<Apikey>;
  /** Get a single author by name (case-insensitive exact match) */
  authorByName?: Maybe<Author>;
  /** Get a single author series by name (case-insensitive exact match) */
  authorSeriesByName?: Maybe<AuthorSeries>;
  /** Get a paginated list of authors with optional search filter */
  authors: PaginatedAuthorResponse;
  /** Get a club book by ID */
  bookClubBook: BookClubBook;
  bookClubById: BookClub;
  bookClubBySlug?: Maybe<BookClub>;
  /** Get a discussion by ID */
  bookClubDiscussion: BookClubDiscussion;
  /** Get the discussions by the book they're associated with */
  bookClubDiscussionByBook: Array<BookClubDiscussion>;
  /** Get a single message by ID */
  bookClubDiscussionMessage: BookClubDiscussionMessage;
  /** Get messages in a discussion */
  bookClubDiscussionMessages: CursorPaginatedBookClubDiscussionMessageResponse;
  /** Get all discussions for a book club, ordered by pinned first, then by date created */
  bookClubDiscussions: Array<BookClubDiscussion>;
  /** Get a single suggestion by ID */
  bookClubSuggestion: BookClubBookSuggestion;
  /** Get all suggestions for a book club */
  bookClubSuggestions: Array<BookClubBookSuggestion>;
  bookClubs: Array<BookClub>;
  /** Get all bookmarks for a single epub by its media ID */
  bookmarksByMediaId: Array<Bookmark>;
  /** List the custom emojis available on this server */
  customEmojis: Array<CustomEmoji>;
  /**
   * The books in *Deleted books* (removed from their library, not yet deleted
   * permanently) in the libraries the user can see, most recently removed first
   */
  deletedBooks: Array<DeletedBook>;
  /**
   * How long removed books are kept and when they are deleted permanently. Users who may
   * remove books see the retention; library managers also see the trash folder and the
   * last automatic purge
   */
  deletedBooksSettings: DeletedBooksSettings;
  duplicateMedia: Array<Media>;
  emailDeviceById?: Maybe<RegisteredEmailDevice>;
  emailDevices: Array<RegisteredEmailDevice>;
  emailerById?: Maybe<Emailer>;
  emailers: Array<Emailer>;
  /** Get a single epub by its media ID */
  epubById: Epub;
  /**
   * Descriptions for many books, for scripts: the books asked by `ids` and `paths` (as
   * Stump stores them), in that order, one result each. With `onlyMissing` (the default)
   * a book whose summary has 15 words or more is not searched (`HAS_SUMMARY`); a book whose
   * summary is locked (set or cleared by hand) never is (`LOCKED`). Nothing is saved:
   * review the `candidates`, then save with `editMediaMetadataBatch`. Hardcover is asked
   * as background work: within its daily budget for background work, behind people's
   * lookups (about one request every 4 seconds, 2 or 3 a book); when the budget is used
   * up, books report `RETRY_LATER`. At most 25 books per call
   */
  findMediaDescriptions: Array<MediaDescriptions>;
  finishedReadingSessionCount: Scalars['Int']['output'];
  getNotifierById: Notifier;
  getNotifiers: Array<Notifier>;
  /**
   * The files Stump holds in its trash folder for the retention (originals replaced by
   * post-processing, versions replaced by a put-back) in the libraries the user can see,
   * most recent first
   */
  heldFiles: Array<HeldFile>;
  /** Existing author folders (no `seriesKind`) or series folders, for the folder picker */
  intakeFolders: Array<IntakeFolder>;
  intakeItem?: Maybe<IntakeItem>;
  /**
   * The items of the user's "Add books" inbox, newest first. By default everything but
   * discarded items.
   */
  intakeItems: Array<IntakeItem>;
  /** The libraries the user can add books to */
  intakeLibraries: Array<Library>;
  /** The tags the library uses (most used first), for the tag picker */
  intakeVocabulary: Array<IntakeVocabularyEntry>;
  jobById?: Maybe<Job>;
  jobs: PaginatedJobResponse;
  keepReading: PaginatedMediaResponse;
  lastVisitedLibrary?: Maybe<Library>;
  libraries: PaginatedLibraryResponse;
  /** Returns the available alphabet for all libraries in the server */
  librariesAlphabet: Scalars['JSONObject']['output'];
  librariesStats: LibraryStats;
  libraryById?: Maybe<Library>;
  /** How the intake files books into a library (its defaults when never changed) */
  libraryIntakeSettings: LibraryIntakeSettings;
  libraryMissingEntities: PaginatedMissingEntityResponse;
  /** A library's post-processing settings and task counts */
  libraryPostProcessing: LibraryPostProcessing;
  listDirectory: PaginatedDirectoryListingResponse;
  /**
   * Get information about the Stump log file, located at STUMP_CONFIG_DIR/Stump.log, or
   * ~/.stump/Stump.log by default. Information such as the file size, last modified date, etc.
   */
  logfileInfo: LogFileInfo;
  loginActivity: Array<UserLoginActivity>;
  loginActivityById: Array<UserLoginActivity>;
  logs: PaginatedLogResponse;
  me: User;
  media: PaginatedMediaResponse;
  /** Returns the available alphabet for all media in the server */
  mediaAlphabet: Scalars['JSONObject']['output'];
  mediaById?: Maybe<Media>;
  mediaByPath?: Maybe<Media>;
  /**
   * The books at these paths (as Stump stores them), in the order asked: `media` is null
   * for a path with no book the user can see. For scripts that know their files by path,
   * e.g. before `editMediaMetadataBatch`. At most 500 paths per call
   */
  mediaByPaths: Array<MediaPathMatch>;
  mediaCount: Scalars['Int']['output'];
  /**
   * Descriptions for a book from the online sources (Hardcover first, then Open Library
   * and Google Books), for its summary in *Edit details*. Nothing is saved
   */
  mediaDescriptionSuggestions: MediaDescriptions;
  mediaDiskUsage: Scalars['Int']['output'];
  /**
   * Tags for a book already in the library: from its author's and series' other books,
   * books with a similar description and (when `online`, else the library's intake
   * setting) Open Library / Google Books subjects, with its current tags judged against
   * the same evidence
   */
  mediaGenreSuggestions: MediaGenreSuggestions;
  /** The latest saves of a book's metadata edit history, newest first (20 by default) */
  mediaMetadataEdits: Array<MetadataEditSave>;
  /**
   * The distinct metadata values (genres, writers, ...) of the books visible to the
   * user, optionally restricted to a series or to the books of a smart list
   */
  mediaMetadataOverview: MediaMetadataOverview;
  metadataFetchRecord?: Maybe<MetadataFetchRecord>;
  /** Whether scans fetch metadata for the books they add */
  metadataFetchSettings: MetadataFetchSettings;
  metadataProviderConfigById?: Maybe<MetadataProviderConfigModel>;
  metadataProviderConfigs: Array<MetadataProviderConfigModel>;
  /**
   * List the distinct values of a metadata field with their book counts, e.g. every
   * genre and how many books have it
   */
  metadataValueCounts: Array<MetadataValueCount>;
  /** Get all pending invitations for the current user */
  myBookClubInvitations: Array<BookClubInvitation>;
  /**
   * The signed-in user's reading settings as the web reader saved them (a JSON text), or
   * null before the first save
   */
  myReaderSettings?: Maybe<Scalars['String']['output']>;
  /** Reading stats for the authenticated user within the provided range */
  myReadingStats: ReadingStats;
  /** The current streaks held for the authenticated user */
  myReadingStreaks: ReadingStreaks;
  /**
   * A paginated list of reading sessions and events for the authenticated user,
   * for all reading activity across all books
   */
  myReadingTimeline: CursorPaginatedGlobalReadingTimelineNodeResponse;
  numberOfLibraries: Scalars['Int']['output'];
  numberOfSeries: Scalars['Int']['output'];
  onDeck: PaginatedMediaResponse;
  /** The online lookups' settings (whether a Google Books API key is set) */
  onlineLookupSettings: OnlineLookupSettings;
  /** Return all metadata fetch records that are awaiting user review. */
  pendingMetadataMatches: Array<MetadataFetchRecord>;
  /** Post-processing tasks, most recently updated first */
  postProcessingTasks: Array<PostProcessingTaskModel>;
  previousBookClubDiscussions: Array<BookClubDiscussion>;
  /** The books deleted permanently, most recent first (a log: their files were deleted) */
  purgedBooks: Array<PurgedBook>;
  /**
   * Retrieves a reading list by ID for the current user.
   *
   * # Returns
   * A reading list with the given ID. If no reading list with this ID exists for the current user, an error will be returned.
   */
  readingListById: ReadingList;
  /**
   * Retrieves a paginated list of reading lists for the current user.
   *
   * # Returns
   *
   * A paginated list of reading lists.
   */
  readingLists: PaginatedReadingListResponse;
  /**
   * Finds a reading session by its ID, if it exists. Access control is enforced such that:
   * - The session owner (i.e., the reader) can always access their own session
   * - Other users can access the session if there are any progress-sharing rules in play
   * (e.g., the session owner and the viewer share a book club membership with it enabled)
   */
  readingSessionById?: Maybe<ReadingSession>;
  readingSessionConflictView: ReadingSessionConflictResolutionView;
  /** Books filed through the intake, newest first (who added what, when, where) */
  recentlyAddedBooks: Array<RecentlyAddedBook>;
  recentlyAddedMedia: PaginatedMediaResponse;
  recentlyAddedSeries: PaginatedSeriesResponse;
  /**
   * Books removed from their library (soft-deleted; the files are still on disk), most
   * recently removed first. They can be brought back with `restoreMedia`.
   */
  removedMedia: Array<Media>;
  scheduledJobs: Array<ScheduledJob>;
  series: PaginatedSeriesResponse;
  /** Returns the available alphabet for all series in the server */
  seriesAlphabet: Scalars['JSONObject']['output'];
  seriesById?: Maybe<Series>;
  serverConfig: ServerConfigModel;
  smartListById?: Maybe<SmartList>;
  smartListItems: SmartListItems;
  smartListMeta?: Maybe<SmartListMeta>;
  smartListViews: Array<SmartListView>;
  smartLists: Array<SmartList>;
  stumpConfig: StumpConfig;
  /** Returns a list of all tags. */
  tags: Array<Tag>;
  topReaders: Array<User>;
  uploadConfig: UploadConfig;
  userById: User;
  userCount: Scalars['Int']['output'];
  users: PaginatedUserResponse;
};


export type QueryAnnotationsByMediaIdArgs = {
  id: Scalars['ID']['input'];
};


export type QueryApiKeyByIdArgs = {
  id: Scalars['Int']['input'];
};


export type QueryAuthorByNameArgs = {
  libraryId?: InputMaybe<Scalars['String']['input']>;
  name: Scalars['String']['input'];
};


export type QueryAuthorSeriesByNameArgs = {
  libraryId?: InputMaybe<Scalars['String']['input']>;
  name: Scalars['String']['input'];
};


export type QueryAuthorsArgs = {
  libraryId?: InputMaybe<Scalars['String']['input']>;
  pagination?: Pagination;
  search?: InputMaybe<Scalars['String']['input']>;
};


export type QueryBookClubBookArgs = {
  id: Scalars['ID']['input'];
};


export type QueryBookClubByIdArgs = {
  id: Scalars['ID']['input'];
};


export type QueryBookClubBySlugArgs = {
  slug: Scalars['String']['input'];
};


export type QueryBookClubDiscussionArgs = {
  id: Scalars['ID']['input'];
};


export type QueryBookClubDiscussionByBookArgs = {
  bookClubBookId: Scalars['ID']['input'];
};


export type QueryBookClubDiscussionMessageArgs = {
  id: Scalars['ID']['input'];
};


export type QueryBookClubDiscussionMessagesArgs = {
  discussionId: Scalars['ID']['input'];
  pagination?: CursorPagination;
  parentId?: InputMaybe<Scalars['ID']['input']>;
};


export type QueryBookClubDiscussionsArgs = {
  bookClubId: Scalars['ID']['input'];
};


export type QueryBookClubSuggestionArgs = {
  suggestionId: Scalars['ID']['input'];
};


export type QueryBookClubSuggestionsArgs = {
  bookClubId: Scalars['ID']['input'];
  status?: InputMaybe<BookClubSuggestionStatus>;
};


export type QueryBookClubsArgs = {
  all?: InputMaybe<Scalars['Boolean']['input']>;
};


export type QueryBookmarksByMediaIdArgs = {
  id: Scalars['ID']['input'];
};


export type QueryDeletedBooksArgs = {
  libraryId?: InputMaybe<Scalars['ID']['input']>;
};


export type QueryEmailDeviceByIdArgs = {
  id: Scalars['Int']['input'];
};


export type QueryEmailerByIdArgs = {
  id: Scalars['Int']['input'];
};


export type QueryEpubByIdArgs = {
  id: Scalars['ID']['input'];
};


export type QueryFindMediaDescriptionsArgs = {
  ids?: Array<Scalars['ID']['input']>;
  onlyMissing?: Scalars['Boolean']['input'];
  paths?: Array<Scalars['String']['input']>;
};


export type QueryGetNotifierByIdArgs = {
  id: Scalars['Int']['input'];
};


export type QueryHeldFilesArgs = {
  libraryId?: InputMaybe<Scalars['ID']['input']>;
};


export type QueryIntakeFoldersArgs = {
  libraryId: Scalars['ID']['input'];
  limit?: Scalars['Int']['input'];
  search?: InputMaybe<Scalars['String']['input']>;
  seriesKind?: InputMaybe<IntakeSeriesKind>;
};


export type QueryIntakeItemArgs = {
  id: Scalars['ID']['input'];
};


export type QueryIntakeItemsArgs = {
  states?: InputMaybe<Array<IntakeState>>;
};


export type QueryIntakeVocabularyArgs = {
  libraryId: Scalars['ID']['input'];
  limit?: Scalars['Int']['input'];
  search?: InputMaybe<Scalars['String']['input']>;
};


export type QueryJobByIdArgs = {
  id: Scalars['ID']['input'];
};


export type QueryJobsArgs = {
  pagination?: Pagination;
};


export type QueryKeepReadingArgs = {
  pagination?: Pagination;
};


export type QueryLibrariesArgs = {
  orderBy?: Array<LibraryModelOrderBy>;
  pagination?: Pagination;
  search?: InputMaybe<Scalars['String']['input']>;
};


export type QueryLibraryByIdArgs = {
  id: Scalars['ID']['input'];
};


export type QueryLibraryIntakeSettingsArgs = {
  libraryId: Scalars['ID']['input'];
};


export type QueryLibraryMissingEntitiesArgs = {
  libraryId: Scalars['ID']['input'];
  pagination?: Pagination;
};


export type QueryLibraryPostProcessingArgs = {
  libraryId: Scalars['ID']['input'];
};


export type QueryListDirectoryArgs = {
  input?: InputMaybe<DirectoryListingInput>;
  pagination: Pagination;
};


export type QueryLoginActivityByIdArgs = {
  id: Scalars['ID']['input'];
};


export type QueryLogsArgs = {
  filter?: LogFilterInput;
  orderBy?: Array<LogModelOrderBy>;
  pagination?: Pagination;
};


export type QueryMediaArgs = {
  filter?: MediaFilterInput;
  orderBy?: Array<MediaOrderBy>;
  pagination?: Pagination;
};


export type QueryMediaByIdArgs = {
  id: Scalars['ID']['input'];
};


export type QueryMediaByPathArgs = {
  path: Scalars['String']['input'];
};


export type QueryMediaByPathsArgs = {
  paths: Array<Scalars['String']['input']>;
};


export type QueryMediaDescriptionSuggestionsArgs = {
  id: Scalars['ID']['input'];
};


export type QueryMediaGenreSuggestionsArgs = {
  id: Scalars['ID']['input'];
  online?: InputMaybe<Scalars['Boolean']['input']>;
};


export type QueryMediaMetadataEditsArgs = {
  limit?: InputMaybe<Scalars['Int']['input']>;
  mediaId: Scalars['ID']['input'];
};


export type QueryMediaMetadataOverviewArgs = {
  seriesId?: InputMaybe<Scalars['ID']['input']>;
  smartListId?: InputMaybe<Scalars['ID']['input']>;
};


export type QueryMetadataFetchRecordArgs = {
  id: MetadataFetchRecordId;
};


export type QueryMetadataProviderConfigByIdArgs = {
  id: Scalars['Int']['input'];
};


export type QueryMetadataValueCountsArgs = {
  field: MetadataFacetField;
  libraryId?: InputMaybe<Scalars['ID']['input']>;
  search?: InputMaybe<Scalars['String']['input']>;
};


export type QueryMyReadingStatsArgs = {
  range?: DateRangeInput;
};


export type QueryMyReadingTimelineArgs = {
  order?: OrderDirection;
  pagination?: CursorPagination;
};


export type QueryOnDeckArgs = {
  pagination?: Pagination;
};


export type QueryPostProcessingTasksArgs = {
  kind?: InputMaybe<PostProcessKind>;
  libraryId?: InputMaybe<Scalars['ID']['input']>;
  limit?: Scalars['Int']['input'];
  statuses?: InputMaybe<Array<PostProcessStatus>>;
};


export type QueryPreviousBookClubDiscussionsArgs = {
  bookClubId: Scalars['ID']['input'];
};


export type QueryPurgedBooksArgs = {
  limit?: Scalars['Int']['input'];
};


export type QueryReadingListByIdArgs = {
  id: Scalars['ID']['input'];
};


export type QueryReadingListsArgs = {
  pagination?: Pagination;
};


export type QueryReadingSessionByIdArgs = {
  id: Scalars['Int']['input'];
};


export type QueryReadingSessionConflictViewArgs = {
  branchedSessionId?: InputMaybe<Scalars['Int']['input']>;
  mediaId: Scalars['ID']['input'];
};


export type QueryRecentlyAddedBooksArgs = {
  limit?: Scalars['Int']['input'];
  offset?: Scalars['Int']['input'];
  userId?: InputMaybe<Scalars['ID']['input']>;
};


export type QueryRecentlyAddedMediaArgs = {
  pagination?: Pagination;
};


export type QueryRecentlyAddedSeriesArgs = {
  pagination?: Pagination;
};


export type QueryRemovedMediaArgs = {
  libraryId?: InputMaybe<Scalars['ID']['input']>;
};


export type QuerySeriesArgs = {
  filter?: SeriesFilterInput;
  orderBy?: Array<SeriesOrderBy>;
  pagination?: Pagination;
};


export type QuerySeriesByIdArgs = {
  id: Scalars['ID']['input'];
};


export type QuerySmartListByIdArgs = {
  id: Scalars['ID']['input'];
};


export type QuerySmartListItemsArgs = {
  filter?: InputMaybe<MediaFilterInput>;
  grouping?: InputMaybe<SmartListGrouping>;
  id: Scalars['ID']['input'];
  limit?: InputMaybe<Scalars['Int']['input']>;
};


export type QuerySmartListMetaArgs = {
  id: Scalars['ID']['input'];
};


export type QuerySmartListsArgs = {
  input?: SmartListsInput;
};


export type QueryTopReadersArgs = {
  take?: InputMaybe<Scalars['Int']['input']>;
};


export type QueryUserByIdArgs = {
  id: Scalars['ID']['input'];
};


export type QueryUsersArgs = {
  pagination?: Pagination;
};

export type ReadingDeviceModel = {
  __typename?: 'ReadingDeviceModel';
  createdAt: Scalars['DateTime']['output'];
  id: Scalars['String']['output'];
  name: Scalars['String']['output'];
  updatedAt: Scalars['DateTime']['output'];
  userId: Scalars['String']['output'];
};

/** The different reading directions supported by any Stump reader */
export enum ReadingDirection {
  Ltr = 'LTR',
  Rtl = 'RTL'
}

/** The different ways an image may be scaled to fit a reader's viewport */
export enum ReadingImageScaleFit {
  Auto = 'AUTO',
  Height = 'HEIGHT',
  None = 'NONE',
  Width = 'WIDTH'
}

export type ReadingList = {
  __typename?: 'ReadingList';
  /** How many of the list's books the user can see */
  bookCount: Scalars['Int']['output'];
  /** The list's books the user can see, in the list's order */
  books: Array<Media>;
  creatingUserId: Scalars['String']['output'];
  description?: Maybe<Scalars['String']['output']>;
  id: Scalars['String']['output'];
  name: Scalars['String']['output'];
  ordering: Scalars['String']['output'];
  updatedAt: Scalars['DateTime']['output'];
  visibility: Scalars['String']['output'];
};


export type ReadingListBooksArgs = {
  limit?: InputMaybe<Scalars['Int']['input']>;
};

export type ReadingListInput = {
  description?: InputMaybe<Scalars['String']['input']>;
  /** The list's ID, generated when omitted or empty */
  id?: InputMaybe<Scalars['String']['input']>;
  /** The list's first books, in order */
  mediaIds?: Array<Scalars['String']['input']>;
  name: Scalars['String']['input'];
  /** Private (only its owner sees it) when omitted */
  visibility?: InputMaybe<EntityVisibility>;
};

/** The different reading modes supported by any Stump reader */
export enum ReadingMode {
  ContinuousHorizontal = 'CONTINUOUS_HORIZONTAL',
  ContinuousVertical = 'CONTINUOUS_VERTICAL',
  Paged = 'PAGED'
}

export type ReadingSession = {
  __typename?: 'ReadingSession';
  /**
   * Returns the list of chapter titles read during this session, including the start
   * and end chapters.
   *
   * ## Important: This has some io cost and so should not necessarily be used in a list
   * of sessions but more a detail view into a single session, etc.
   */
  chaptersRead: Array<Scalars['String']['output']>;
  createdAt: Scalars['DateTime']['output'];
  deviceIds: Array<Scalars['String']['output']>;
  /** The devices which were used during this reading session */
  devices: Array<ReadingDeviceModel>;
  /** accumulated reading time for this session, updated via deltas (not overwritten) */
  elapsedSeconds?: Maybe<Scalars['Int']['output']>;
  endLocator?: Maybe<ReadiumLocator>;
  endPage?: Maybe<Scalars['Int']['output']>;
  endPercentage?: Maybe<Scalars['Decimal']['output']>;
  events: Array<SessionEvent>;
  id: Scalars['Int']['output'];
  isLatest: Scalars['Boolean']['output'];
  journalEntry?: Maybe<JournalEntryModel>;
  koreaderProgress?: Maybe<Scalars['String']['output']>;
  /**
   * The media which this session belongs to. Please note that if somehow the user loses access to the
   * media record, e.g. via access control, then this will resolve to `None` to avoid leaking
   * information which the user no longer has access to. It is a bit awkward, since the session itself
   * means they have at least at some point read some portion
   */
  media?: Maybe<Media>;
  mediaId: Scalars['String']['output'];
  /** which read-through of this book this session belongs to (1-indexed) */
  readthroughNumber: Scalars['Int']['output'];
  /** the "logical" date of this session, based on user prefs and start time */
  sessionDate: Scalars['NaiveDate']['output'];
  startLocator?: Maybe<ReadiumLocator>;
  startPage?: Maybe<Scalars['Int']['output']>;
  startPercentage?: Maybe<Scalars['Decimal']['output']>;
  /**
   * the status of this session. this might feel confusing when considering that sessions will
   * remain in place even after completion/dnf, but the idea is that the status represents the
   * state of the session when it was last updated
   */
  status: ReadingStatus;
  updatedAt?: Maybe<Scalars['DateTime']['output']>;
  userId: Scalars['String']['output'];
};


export type ReadingSessionEventsArgs = {
  order?: OrderDirection;
};

/**
 * a view through which a client can resolve conflicts relative to a local ancestor session
 * and any number of remote sessions which were created afterwards
 */
export type ReadingSessionConflictResolutionView = {
  __typename?: 'ReadingSessionConflictResolutionView';
  /**
   * the last session which was known to be in sync with the local client. it's possible there is no ancestor session, e.g. if
   * the book was downloaded on the client before any reading sessions were created on the server
   */
  ancestorSession?: Maybe<ReadingSession>;
  /**
   * all sessions created/updated on **this server** (remote) after the ancestor session, ordered
   * by created_at ascending
   */
  remoteSessions: Array<ReadingSession>;
};

export type ReadingStats = {
  __typename?: 'ReadingStats';
  /** One entry per day with at least one session */
  dailyActivity: Array<DailyActivity>;
  /** Average reading time for each weekday */
  dayOfWeek: Array<WeekdayActivity>;
  range: ResolvedDateRange;
  /** Median active time and days to finish a book per library type */
  timeToFinish: Array<TimeToFinishStat>;
  totals: ReadingTotals;
};

/**
 * the different reading statuses a book can be categorized as based on a user's
 * reading sessions
 */
export enum ReadingStatus {
  /** a user actively started reading a book but decided not to finish it (i.e., dnf-ing a book) */
  Abandoned = 'ABANDONED',
  /** there is at least one completed readthrough for this book */
  Finished = 'FINISHED',
  /** no sessions have been recorded for this book */
  NotStarted = 'NOT_STARTED',
  /**
   * there is an active reading session for this book. it may or may not have been completed in
   * the past, this is strictly about the presence of an active session
   */
  Reading = 'READING'
}

export type ReadingStreaks = {
  __typename?: 'ReadingStreaks';
  currentDays: Scalars['Int']['output'];
  longestDays: Scalars['Int']['output'];
};

/** The summed stats relative to the aggregation range */
export type ReadingTotals = {
  __typename?: 'ReadingTotals';
  activeDayCount: Scalars['Int']['output'];
  dnfCount: Scalars['Int']['output'];
  finishedCount: Scalars['Int']['output'];
  sessionCount: Scalars['Int']['output'];
  totalSeconds: Scalars['Int']['output'];
};

export type ReadiumLocation = {
  __typename?: 'ReadiumLocation';
  cssSelector?: Maybe<Scalars['String']['output']>;
  fragments?: Maybe<Array<Scalars['String']['output']>>;
  partialCfi?: Maybe<Scalars['String']['output']>;
  position?: Maybe<Scalars['Int']['output']>;
  progression?: Maybe<Scalars['Decimal']['output']>;
  /**
   * The boxes a selection covers on a fixed-layout page (e.g. a PDF page, whose number is
   * `position`), so an annotation can be drawn without re-finding its text. Not part of the
   * Readium spec, unused for reflowable EPUBs
   */
  rects?: Maybe<Array<ReadiumRect>>;
  totalProgression?: Maybe<Scalars['Decimal']['output']>;
};

export type ReadiumLocationInput = {
  cssSelector?: InputMaybe<Scalars['String']['input']>;
  fragments?: InputMaybe<Array<Scalars['String']['input']>>;
  partialCfi?: InputMaybe<Scalars['String']['input']>;
  position?: InputMaybe<Scalars['Int']['input']>;
  progression?: InputMaybe<Scalars['Decimal']['input']>;
  /**
   * The boxes a selection covers on a fixed-layout page (e.g. a PDF page, whose number is
   * `position`), so an annotation can be drawn without re-finding its text. Not part of the
   * Readium spec, unused for reflowable EPUBs
   */
  rects?: InputMaybe<Array<ReadiumRectInput>>;
  totalProgression?: InputMaybe<Scalars['Decimal']['input']>;
};

export type ReadiumLocator = {
  __typename?: 'ReadiumLocator';
  chapterTitle: Scalars['String']['output'];
  href: Scalars['String']['output'];
  locations?: Maybe<ReadiumLocation>;
  text?: Maybe<ReadiumText>;
  title?: Maybe<Scalars['String']['output']>;
  type: Scalars['String']['output'];
};

export type ReadiumLocatorInput = {
  chapterTitle?: Scalars['String']['input'];
  href: Scalars['String']['input'];
  locations?: InputMaybe<ReadiumLocationInput>;
  text?: InputMaybe<ReadiumTextInput>;
  title?: InputMaybe<Scalars['String']['input']>;
  type?: Scalars['String']['input'];
};

/**
 * A box on a fixed-layout page, as fractions (0-1) of the page's width and height from its
 * top-left corner, so it stays put at any zoom
 */
export type ReadiumRect = {
  __typename?: 'ReadiumRect';
  height: Scalars['Decimal']['output'];
  width: Scalars['Decimal']['output'];
  x: Scalars['Decimal']['output'];
  y: Scalars['Decimal']['output'];
};

/**
 * A box on a fixed-layout page, as fractions (0-1) of the page's width and height from its
 * top-left corner, so it stays put at any zoom
 */
export type ReadiumRectInput = {
  height: Scalars['Decimal']['input'];
  width: Scalars['Decimal']['input'];
  x: Scalars['Decimal']['input'];
  y: Scalars['Decimal']['input'];
};

export type ReadiumText = {
  __typename?: 'ReadiumText';
  after?: Maybe<Scalars['String']['output']>;
  before?: Maybe<Scalars['String']['output']>;
  highlight?: Maybe<Scalars['String']['output']>;
};

export type ReadiumTextInput = {
  after?: InputMaybe<Scalars['String']['input']>;
  before?: InputMaybe<Scalars['String']['input']>;
  highlight?: InputMaybe<Scalars['String']['input']>;
};

/**
 * a completed readthrough of a book, aggregated across all sessions that
 * share the same `readthrough_number`
 */
export type ReadthroughRecord = {
  __typename?: 'ReadthroughRecord';
  completedAt: Scalars['DateTime']['output'];
  dnf: Scalars['Boolean']['output'];
  elapsedSeconds: Scalars['Int']['output'];
  readthroughNumber: Scalars['Int']['output'];
  startedAt: Scalars['DateTime']['output'];
};

/** The timeline of events for a specific readthrough of a book */
export type ReadthroughTimeline = {
  __typename?: 'ReadthroughTimeline';
  finishedAt?: Maybe<Scalars['DateTime']['output']>;
  readthroughNumber: Scalars['Int']['output'];
  sessions: Array<SessionWithEvents>;
  startedAt: Scalars['DateTime']['output'];
  status: ReadingStatus;
  totalElapsedSeconds: Scalars['Int']['output'];
};

export type RecentlyAdded = {
  __typename?: 'RecentlyAdded';
  entity: FilterableArrangementEntity;
  name?: Maybe<Scalars['String']['output']>;
};

/** A book filed through the intake, for the admin "Recently added" page */
export type RecentlyAddedBook = {
  __typename?: 'RecentlyAddedBook';
  authors: Array<Scalars['String']['output']>;
  filedAt?: Maybe<Scalars['DateTime']['output']>;
  itemId: Scalars['ID']['output'];
  libraryId: Scalars['String']['output'];
  libraryName: Scalars['String']['output'];
  /** The library books created from it (empty until the scan has run) */
  mediaIds: Array<Scalars['String']['output']>;
  /** Relative to the library root */
  paths: Array<Scalars['String']['output']>;
  series?: Maybe<Scalars['String']['output']>;
  title: Scalars['String']['output'];
  userId: Scalars['String']['output'];
  username?: Maybe<Scalars['String']['output']>;
};

export type RecentlyAddedInput = {
  entity: FilterableArrangementEntity;
  name?: InputMaybe<Scalars['String']['input']>;
};

export type RegisteredEmailDevice = {
  __typename?: 'RegisteredEmailDevice';
  email: Scalars['String']['output'];
  forbidden: Scalars['Boolean']['output'];
  id: Scalars['Int']['output'];
  name: Scalars['String']['output'];
  sendHistory: Array<EmailerSendRecord>;
};

export type RescanBooksOutput = {
  __typename?: 'RescanBooksOutput';
  /** Books that are no longer in Stump */
  missingBooks: Scalars['Int']['output'];
  /** Books read again from their files */
  rescannedBooks: Scalars['Int']['output'];
  /** Books whose stored data changed */
  updatedBooks: Scalars['Int']['output'];
};

/** A `DateRangeInput` where `end` is always set */
export type ResolvedDateRange = {
  __typename?: 'ResolvedDateRange';
  end: Scalars['NaiveDate']['output'];
  start?: Maybe<Scalars['NaiveDate']['output']>;
};

/**
 * the current reading position for a book, derived from the latest session
 * with the highest `readthrough_number`
 */
export type ResumeReadingCursor = {
  __typename?: 'ResumeReadingCursor';
  /** total reading time across all sessions in the current readthrough */
  elapsedSeconds: Scalars['Int']['output'];
  locator?: Maybe<ReadiumLocator>;
  page?: Maybe<Scalars['Int']['output']>;
  percentageCompleted?: Maybe<Scalars['Decimal']['output']>;
  /**
   * A page number computed from the current locator's `total_progression` relative
   * to the computed positions list for the book.
   */
  positionAwarePage?: Maybe<Scalars['Int']['output']>;
  readthroughNumber: Scalars['Int']['output'];
  /** the id of the session this cursor is derived from */
  sessionId: Scalars['Int']['output'];
  /** when the very first session in the current readthrough started */
  startedAt?: Maybe<Scalars['DateTime']['output']>;
  /** the last time the latest session in the current readthrough was updated */
  updatedAt?: Maybe<Scalars['DateTime']['output']>;
};

export type SaveSmartListInput = {
  defaultGrouping: SmartListGrouping;
  description?: InputMaybe<Scalars['String']['input']>;
  filters: Array<SmartListFilterGroupInput>;
  joiner: SmartListJoiner;
  name: Scalars['String']['input'];
  visibility: EntityVisibility;
};

export type SaveSmartListView = {
  bookColumns: Array<SmartListViewColumnInput>;
  bookSorting: Array<SmartListViewSortInput>;
  enableMultiSort?: InputMaybe<Scalars['Boolean']['input']>;
  groupColumns: Array<SmartListViewColumnInput>;
  groupSorting: Array<SmartListViewSortInput>;
  listId: Scalars['ID']['input'];
  name: Scalars['String']['input'];
  search?: InputMaybe<Scalars['String']['input']>;
};

export type ScaleEvenlyByFactor = {
  __typename?: 'ScaleEvenlyByFactor';
  /**
   * The factor to scale the image by. Note that this was made a [Decimal]
   * to correct precision issues
   */
  factor: Scalars['Decimal']['output'];
};

export type ScaleEvenlyByFactorInput = {
  /**
   * The factor to scale the image by. Note that this was made a [Decimal]
   * to correct precision issues
   */
  factor: Scalars['Decimal']['input'];
};

/**
 * A resize option which will resize the image while maintaining the aspect ratio.
 * The dimension *not* specified will be calculated based on the aspect ratio.
 */
export type ScaledDimensionResize = {
  __typename?: 'ScaledDimensionResize';
  /** The dimension to set with the given size, e.g. `Height` or `Width`. */
  dimension: Dimension;
  /** The size (in pixels) to set the specified dimension to. */
  size: Scalars['Int']['output'];
};

/**
 * A resize option which will resize the image while maintaining the aspect ratio.
 * The dimension *not* specified will be calculated based on the aspect ratio.
 */
export type ScaledDimensionResizeInput = {
  /** The dimension to set with the given size, e.g. `Height` or `Width`. */
  dimension: Dimension;
  /** The size (in pixels) to set the specified dimension to. */
  size: Scalars['Int']['input'];
};

export type ScheduledJob = {
  __typename?: 'ScheduledJob';
  config?: Maybe<Scalars['JSON']['output']>;
  createdAt: Scalars['DateTime']['output'];
  enabled: Scalars['Boolean']['output'];
  id: Scalars['Int']['output'];
  kind: ScheduledJobKind;
  lastRunAt?: Maybe<Scalars['DateTime']['output']>;
  name: Scalars['String']['output'];
  schedule: Scalars['String']['output'];
};

/** A oneOf input for the schedule config */
export type ScheduledJobConfigInput =
  { libraryScan: LibraryScanConfigInput; metadataRetry?: never; }
  |  { libraryScan?: never; metadataRetry: MetadataRetryConfigInput; };

/** The kind of a scheduled job, aligned with the config variants */
export enum ScheduledJobKind {
  /** Scan one or more libraries on a cron schedule */
  LibraryScan = 'LIBRARY_SCAN',
  /** Retry fetching metadata for records that were rate-limited or failed */
  MetadataRetry = 'METADATA_RETRY'
}

export type SendAttachmentEmailOutput = {
  __typename?: 'SendAttachmentEmailOutput';
  errors: Array<Scalars['String']['output']>;
  sentCount: Scalars['Int']['output'];
};

export type SendAttachmentEmailsInput = {
  mediaIds: Array<Scalars['ID']['input']>;
  sendTo: Array<EmailerSendTo>;
};

export type SendMessageInput = {
  content: Scalars['String']['input'];
  /** The parent message inside a thread, denoting this message as a child */
  parentMessageId?: InputMaybe<Scalars['String']['input']>;
  /** An inline reply reference, NOT a child of a thread */
  replyToMessageId?: InputMaybe<Scalars['String']['input']>;
};

export type SendToDevice = {
  id: Scalars['Int']['input'];
};

export type SendToEmail = {
  email: Scalars['String']['input'];
};

export type Series = {
  __typename?: 'Series';
  createdAt: Scalars['DateTime']['output'];
  deletedAt?: Maybe<Scalars['DateTime']['output']>;
  description?: Maybe<Scalars['String']['output']>;
  id: Scalars['String']['output'];
  isComplete: Scalars['Boolean']['output'];
  isFavorite: Scalars['Boolean']['output'];
  isOneshot: Scalars['Boolean']['output'];
  kind: SeriesKind;
  library: Library;
  libraryId?: Maybe<Scalars['String']['output']>;
  /** Get media in this series */
  media: Array<Media>;
  mediaAlphabet: Scalars['JSONObject']['output'];
  mediaCount: Scalars['Int']['output'];
  metadata?: Maybe<SeriesMetadata>;
  name: Scalars['String']['output'];
  oneshotBook?: Maybe<Media>;
  path?: Maybe<Scalars['String']['output']>;
  percentageCompleted: Scalars['Float']['output'];
  readCount: Scalars['Int']['output'];
  resolvedDescription?: Maybe<Scalars['String']['output']>;
  resolvedName: Scalars['String']['output'];
  /**
   * Thumbnails for a stacked series card, in order of:
   *
   * 1. the series thumbnail (e.g., if manually set)
   * 2. the first `take` amount of books, excluding the one which the thumbnail
   * was derived from
   */
  stackedThumbnails: Array<ImageRef>;
  stats: SeriesStats;
  status: FileStatus;
  tags: Array<Tag>;
  /**
   * A reference to the thumbnail image for the thumbnail. This will be a fully
   * qualified URL to the image.
   */
  thumbnail: ImageRef;
  thumbnailMeta?: Maybe<ImageMetadata>;
  thumbnailPath?: Maybe<Scalars['String']['output']>;
  unreadCount: Scalars['Int']['output'];
  upNext: Array<Media>;
  updatedAt?: Maybe<Scalars['DateTime']['output']>;
};


export type SeriesMediaArgs = {
  skip?: InputMaybe<Scalars['Int']['input']>;
  take?: InputMaybe<Scalars['Int']['input']>;
};


export type SeriesStackedThumbnailsArgs = {
  take?: Scalars['Int']['input'];
};


export type SeriesStatsArgs = {
  allUsers?: InputMaybe<Scalars['Boolean']['input']>;
};


export type SeriesUpNextArgs = {
  cursor?: InputMaybe<Scalars['String']['input']>;
  take?: Scalars['Int']['input'];
};

export type SeriesFilterInput = {
  _and?: InputMaybe<Array<SeriesFilterInput>>;
  _not?: InputMaybe<Array<SeriesFilterInput>>;
  _or?: InputMaybe<Array<SeriesFilterInput>>;
  isOneshot?: InputMaybe<Scalars['Boolean']['input']>;
  library?: InputMaybe<LibraryFilterInput>;
  libraryId?: InputMaybe<FieldFilterString>;
  libraryType?: InputMaybe<ComputedFilterLibraryType>;
  metadata?: InputMaybe<SeriesMetadataFilterInput>;
  name?: InputMaybe<FieldFilterString>;
  path?: InputMaybe<FieldFilterString>;
  readingStatus?: InputMaybe<ComputedFilterReadingStatus>;
  /**
   * Free-text search: every word must appear in the series name or title, ignoring case
   * and accents
   */
  search?: InputMaybe<Scalars['String']['input']>;
};

export enum SeriesKind {
  Filesystem = 'FILESYSTEM',
  Virtual = 'VIRTUAL'
}

export type SeriesMetadata = {
  __typename?: 'SeriesMetadata';
  /** Age rating of the series */
  ageRating?: Maybe<Scalars['Int']['output']>;
  /** Booktype of the series (Print, OneShot, TPB or GN) */
  booktype?: Maybe<Scalars['String']['output']>;
  characters: Array<Scalars['String']['output']>;
  collects: Array<CollectedItem>;
  /** Image URL pointing to CV image of series cover (usually issue #1) */
  comicImage?: Maybe<Scalars['String']['output']>;
  /** ComicVine comicid */
  comicid?: Maybe<Scalars['Int']['output']>;
  /** Description (summary) with line breaks, carriage returns, etc. */
  descriptionFormatted?: Maybe<Scalars['String']['output']>;
  genres: Array<Scalars['String']['output']>;
  /** Name of imprint while under publisher */
  imprint?: Maybe<Scalars['String']['output']>;
  links: Array<Scalars['String']['output']>;
  lockedFields: Array<MetadataField>;
  /** Type of series (e.g. "comicSeries") */
  metaType?: Maybe<Scalars['String']['output']>;
  /** The external ID on the metadata provider's system */
  metadataExternalId?: Maybe<Scalars['String']['output']>;
  /** The external metadata provider that supplied this metadata (e.g., "HARDCOVER") */
  metadataSource?: Maybe<Scalars['String']['output']>;
  /** Start and end of the series in "Month Year - Month Year" format. If series status is Continuing, the end value is "Present" */
  publicationRun?: Maybe<Scalars['String']['output']>;
  /** Publisher name */
  publisher?: Maybe<Scalars['String']['output']>;
  seriesId: Scalars['String']['output'];
  /** Either "Continuing" or "Ended" */
  status?: Maybe<Scalars['String']['output']>;
  /**
   * Description taken from source (un-edited) with no line breaks, carriage returns, etc.
   * Stump calls this 'summary' to align with other models, but is derived from 'description_text' in series.json
   */
  summary?: Maybe<Scalars['String']['output']>;
  /** Title of series */
  title?: Maybe<Scalars['String']['output']>;
  /** Total issues in the series up until this point in time */
  totalIssues?: Maybe<Scalars['Int']['output']>;
  /** Volume of the series in relation to other titles (this can be either numerical or the series year) */
  volume?: Maybe<Scalars['Int']['output']>;
  writers: Array<Scalars['String']['output']>;
  /** Year the series started (publication start) */
  year?: Maybe<Scalars['Int']['output']>;
};

export type SeriesMetadataFilterInput = {
  _and?: InputMaybe<Array<SeriesMetadataFilterInput>>;
  _not?: InputMaybe<Array<SeriesMetadataFilterInput>>;
  _or?: InputMaybe<Array<SeriesMetadataFilterInput>>;
  ageRating?: InputMaybe<NumericFilterI32>;
  booktype?: InputMaybe<FieldFilterString>;
  comicid?: InputMaybe<NumericFilterI32>;
  imprint?: InputMaybe<FieldFilterString>;
  metaType?: InputMaybe<FieldFilterString>;
  publisher?: InputMaybe<FieldFilterString>;
  status?: InputMaybe<FieldFilterString>;
  summary?: InputMaybe<FieldFilterString>;
  title?: InputMaybe<FieldFilterString>;
  volume?: InputMaybe<NumericFilterI32>;
  year?: InputMaybe<NumericFilterI32>;
};

export type SeriesMetadataInput = {
  ageRating?: InputMaybe<Scalars['Int']['input']>;
  booktype?: InputMaybe<Scalars['String']['input']>;
  characters?: InputMaybe<Array<Scalars['String']['input']>>;
  collects?: InputMaybe<Array<CollectedItemInput>>;
  comicImage?: InputMaybe<Scalars['String']['input']>;
  comicid?: InputMaybe<Scalars['Int']['input']>;
  descriptionFormatted?: InputMaybe<Scalars['String']['input']>;
  genres?: InputMaybe<Array<Scalars['String']['input']>>;
  imprint?: InputMaybe<Scalars['String']['input']>;
  links?: InputMaybe<Array<Scalars['String']['input']>>;
  metaType?: InputMaybe<Scalars['String']['input']>;
  publicationRun?: InputMaybe<Scalars['String']['input']>;
  publisher?: InputMaybe<Scalars['String']['input']>;
  status?: InputMaybe<Scalars['String']['input']>;
  summary?: InputMaybe<Scalars['String']['input']>;
  title?: InputMaybe<Scalars['String']['input']>;
  totalIssues?: InputMaybe<Scalars['Int']['input']>;
  volume?: InputMaybe<Scalars['Int']['input']>;
  writers?: InputMaybe<Array<Scalars['String']['input']>>;
  year?: InputMaybe<Scalars['Int']['input']>;
};

export enum SeriesMetadataModelOrdering {
  AgeRating = 'AGE_RATING',
  Booktype = 'BOOKTYPE',
  Characters = 'CHARACTERS',
  Collects = 'COLLECTS',
  Comicid = 'COMICID',
  ComicImage = 'COMIC_IMAGE',
  DescriptionFormatted = 'DESCRIPTION_FORMATTED',
  Genres = 'GENRES',
  Imprint = 'IMPRINT',
  Links = 'LINKS',
  LockedFields = 'LOCKED_FIELDS',
  MetadataExternalId = 'METADATA_EXTERNAL_ID',
  MetadataSource = 'METADATA_SOURCE',
  MetaType = 'META_TYPE',
  PublicationRun = 'PUBLICATION_RUN',
  Publisher = 'PUBLISHER',
  SeriesId = 'SERIES_ID',
  Status = 'STATUS',
  Summary = 'SUMMARY',
  Title = 'TITLE',
  TotalIssues = 'TOTAL_ISSUES',
  Volume = 'VOLUME',
  Writers = 'WRITERS',
  Year = 'YEAR'
}

export type SeriesMetadataOrderByField = {
  direction: OrderDirection;
  field: SeriesMetadataModelOrdering;
};

export enum SeriesModelOrdering {
  CreatedAt = 'CREATED_AT',
  DeletedAt = 'DELETED_AT',
  Description = 'DESCRIPTION',
  Id = 'ID',
  IsOneshot = 'IS_ONESHOT',
  Kind = 'KIND',
  LibraryId = 'LIBRARY_ID',
  Name = 'NAME',
  Path = 'PATH',
  Status = 'STATUS',
  ThumbnailMeta = 'THUMBNAIL_META',
  ThumbnailPath = 'THUMBNAIL_PATH',
  UpdatedAt = 'UPDATED_AT'
}

export type SeriesOrderBy =
  { metadata: SeriesMetadataOrderByField; series?: never; }
  |  { metadata?: never; series: SeriesOrderByField; };

export type SeriesOrderByField = {
  direction: OrderDirection;
  field: SeriesModelOrdering;
};

export type SeriesScanOutput = {
  __typename?: 'SeriesScanOutput';
  /** The number of media entities that were created */
  createdMedia: Scalars['Int']['output'];
  /** The number of files that were ignored during the scan */
  ignoredFiles: Scalars['Int']['output'];
  /**
   * The number of files that were deemed to be skipped during the scan, e.g. it
   * exists in the database but has not been modified since the last scan
   */
  skippedFiles: Scalars['Int']['output'];
  /** The number of files to scan relative to the series root */
  totalFiles: Scalars['Int']['output'];
  /** The number of media entities that were updated */
  updatedMedia: Scalars['Int']['output'];
};

export type SeriesStats = {
  __typename?: 'SeriesStats';
  bookCount: Scalars['Int']['output'];
  completedBooks: Scalars['Int']['output'];
  inProgressBooks: Scalars['Int']['output'];
  totalBytes: Scalars['Int']['output'];
  totalReadingTimeSeconds: Scalars['Int']['output'];
};

export type ServerConfigModel = {
  __typename?: 'ServerConfigModel';
  id: Scalars['Int']['output'];
  initialWalSetupComplete: Scalars['Boolean']['output'];
  publicUrl?: Maybe<Scalars['String']['output']>;
};

export type SessionEvent = Bookmark | MediaAnnotation;

export type SessionWithEvents = {
  __typename?: 'SessionWithEvents';
  events: Array<SessionEvent>;
  session: ReadingSession;
};

/**
 * A work that has multiple authors (co-authored). This wrapper allows querying
 * the authors/co-authors of the work in context.
 */
export type SharedWork = {
  __typename?: 'SharedWork';
  /** All authors who contributed to this work, with their roles */
  authors: Array<Author>;
  /** Authors who contributed to this work, excluding the viewing author */
  coAuthors: Array<Author>;
  /** The media/book itself */
  media: Media;
};

export type SmartList = {
  __typename?: 'SmartList';
  books: Array<Media>;
  creatorId: Scalars['String']['output'];
  defaultGrouping: SmartListGrouping;
  description?: Maybe<Scalars['String']['output']>;
  filters: Scalars['String']['output'];
  id: Scalars['String']['output'];
  items: SmartListItems;
  joiner: SmartListJoiner;
  meta: SmartListMeta;
  name: Scalars['String']['output'];
  thumbnail: ImageRef;
  views: Array<SmartListView>;
  visibility: EntityVisibility;
};


export type SmartListBooksArgs = {
  limit?: InputMaybe<Scalars['Int']['input']>;
};


export type SmartListItemsArgs = {
  limit?: InputMaybe<Scalars['Int']['input']>;
};

export type SmartListFilterGroupInput = {
  groups: Array<SmartListFilterInput>;
  joiner: SmartListGroupJoiner;
};

export type SmartListFilterInput =
  { library: LibraryFilterInput; media?: never; mediaMetadata?: never; series?: never; seriesMetadata?: never; }
  |  { library?: never; media: MediaFilterInput; mediaMetadata?: never; series?: never; seriesMetadata?: never; }
  |  { library?: never; media?: never; mediaMetadata: MediaMetadataFilterInput; series?: never; seriesMetadata?: never; }
  |  { library?: never; media?: never; mediaMetadata?: never; series: SeriesFilterInput; seriesMetadata?: never; }
  |  { library?: never; media?: never; mediaMetadata?: never; series?: never; seriesMetadata: SeriesMetadataFilterInput; };

/** The different filter joiners that can be used in smart lists */
export enum SmartListGroupJoiner {
  And = 'AND',
  Not = 'NOT',
  Or = 'OR'
}

export type SmartListGrouped = {
  __typename?: 'SmartListGrouped';
  items: Array<SmartListGroupedItem>;
};

export type SmartListGroupedItem = {
  __typename?: 'SmartListGroupedItem';
  books: Array<Media>;
  entity?: Maybe<SmartListItemEntity>;
};

/** The different grouping options for smart lists */
export enum SmartListGrouping {
  ByBooks = 'BY_BOOKS',
  ByLibrary = 'BY_LIBRARY',
  BySeries = 'BY_SERIES'
}

export type SmartListItemEntity = Library | Series;

export type SmartListItems = SmartListGrouped | SmartListUngrouped;

/** The different filter joiners that can be used in smart lists */
export enum SmartListJoiner {
  And = 'AND',
  Or = 'OR'
}

export type SmartListMeta = {
  __typename?: 'SmartListMeta';
  matchedBooks: Scalars['Int']['output'];
  matchedLibraries: Scalars['Int']['output'];
  matchedSeries: Scalars['Int']['output'];
};

export type SmartListUngrouped = {
  __typename?: 'SmartListUngrouped';
  books: Array<Media>;
};

export type SmartListView = {
  __typename?: 'SmartListView';
  bookColumns: Array<SmartListViewColumn>;
  bookSorting: Array<SmartListViewSort>;
  enableMultiSort?: Maybe<Scalars['Boolean']['output']>;
  groupColumns: Array<SmartListViewColumn>;
  groupSorting: Array<SmartListViewSort>;
  id: Scalars['Int']['output'];
  listId: Scalars['String']['output'];
  name: Scalars['String']['output'];
  search?: Maybe<Scalars['String']['output']>;
};

export type SmartListViewColumn = {
  __typename?: 'SmartListViewColumn';
  id: Scalars['String']['output'];
  position: Scalars['Int']['output'];
};

export type SmartListViewColumnInput = {
  id: Scalars['String']['input'];
  position: Scalars['Int']['input'];
};

export type SmartListViewSort = {
  __typename?: 'SmartListViewSort';
  desc: Scalars['Boolean']['output'];
  id: Scalars['String']['output'];
};

export type SmartListViewSortInput = {
  desc: Scalars['Boolean']['input'];
  id: Scalars['String']['input'];
};

export type SmartListsInput = {
  all?: InputMaybe<Scalars['Boolean']['input']>;
  mine?: InputMaybe<Scalars['Boolean']['input']>;
  search?: InputMaybe<Scalars['String']['input']>;
};

export type SpineItem = {
  __typename?: 'SpineItem';
  id?: Maybe<Scalars['String']['output']>;
  idref: Scalars['String']['output'];
  linear: Scalars['Boolean']['output'];
  properties?: Maybe<Scalars['String']['output']>;
};

export type StumpConfig = {
  __typename?: 'StumpConfig';
  /** The time in seconds that an access token will be valid for */
  accessTokenTtl: Scalars['Int']['output'];
  /** A comma-separated list of origins for CORS */
  allowedOrigins: Array<Scalars['String']['output']>;
  /**
   * The directory where the web app bundle lives, which the server will serve as
   * static files
   */
  clientDir: Scalars['String']['output'];
  /** Whether or not to include ANSI color codes in log files */
  colorfulLogs: Scalars['Boolean']['output'];
  /** The configuration root for the Stump application */
  configDir: Scalars['String']['output'];
  /** An optional custom path for the database. If set, this assumes SQLite. */
  dbPath?: Maybe<Scalars['String']['output']>;
  /** The timeout in seconds for database connections */
  dbTimeoutSecs: Scalars['Int']['output'];
  /** Indicates if the Kobo sync feature should be enabled */
  enableKoboSync: Scalars['Boolean']['output'];
  /** Indicates if the KoReader sync feature should be enabled */
  enableKoreaderSync: Scalars['Boolean']['output'];
  /**
   * Indicates if OPDS page access should automatically track reading progression.
   * When disabled, clients loading/preloading pages won't trigger progress updates.
   */
  enableOpdsProgression: Scalars['Boolean']['output'];
  /**
   * Indicates if the GraphQL playground should be enabled. If true, the server
   * will allow GET requests to the GraphQL endpoint and serve the playground UI
   */
  enablePlayground: Scalars['Boolean']['output'];
  /**
   * Whether or not the server will allow users with the appropriate permissions
   * to upload books, series, or other valid uploadable content
   */
  enableUpload: Scalars['Boolean']['output'];
  /** The interval in seconds at which expired sessions will be cleaned up */
  expiredSessionCleanupInterval: Scalars['Int']['output'];
  /** The IP address on which to listen on (default: "0.0.0.0") */
  ip: Scalars['String']['output'];
  /**
   * The directory where the applicaiton logs will be stored. If unspecified,
   * logs will be stored in the config_dir
   */
  logDir?: Maybe<Scalars['String']['output']>;
  /** The maximum size in bytes of a file upload */
  maxFileUploadSize: Scalars['Int']['output'];
  /** The maximum size in bytes of an image upload */
  maxImageUploadSize: Scalars['Int']['output'];
  /**
   * A multiplier applied to the number of logical CPUs to derive the default scanner concurrency
   * limit. Increasing can speed things up but will increase resource usage
   */
  parallelismMultiplier: Scalars['Int']['output'];
  /** Password hash cost */
  passwordHashCost: Scalars['Int']['output'];
  /**
   * The size cap of the rendered PDF page cache in MB (0 = no cap). The oldest pages are
   * removed when it is reached
   */
  pdfCacheMaxMb: Scalars['Int']['output'];
  /** Whether to enable disk caching for rendered PDF pages */
  pdfCachePages: Scalars['Boolean']['output'];
  /** Whether to enable high-quality rendering with smoothing (slower but better quality) */
  pdfHighQuality: Scalars['Boolean']['output'];
  /** The maximum width or height dimension for rendered PDF pages */
  pdfMaxDimension: Scalars['Int']['output'];
  /**
   * The widest page picture (px) the PDF reader may request: 720, 1000, 1400, 2000 or 2800.
   * Wider pictures are sharper on big HiDPI screens but take more memory to render
   */
  pdfMaxTier: Scalars['Int']['output'];
  /** Number of pages to pre-render before and after the current page */
  pdfPrerenderRange: Scalars['Int']['output'];
  /** The DPI (dots per inch) to use when rendering PDF pages as images */
  pdfRenderDpi: Scalars['Int']['output'];
  /** The image format to use for rendered PDF pages (webp, png, jpeg) */
  pdfRenderFormat: Scalars['String']['output'];
  /** Path to the PDFium binary for enabling PDF support */
  pdfiumPath?: Maybe<Scalars['String']['output']>;
  /** The port from which to serve the application (default: 10801) */
  port: Scalars['Int']['output'];
  /** Whether or not to pretty print logs */
  prettyLogs: Scalars['Boolean']['output'];
  /**
   * No longer used (kept so an existing `Stump.toml` or `STUMP_QUARANTINE_DIR` still loads):
   * the originals post-processing replaces are kept in `<config_dir>/trash` and listed in
   * *Deleted books*, and books deleted permanently are deleted. Files already in that folder
   * are left alone
   */
  quarantineDir?: Maybe<Scalars['String']['output']>;
  /** The time in seconds that a refresh token will be valid for */
  refreshTokenTtl: Scalars['Int']['output'];
  /** The time in seconds that a login session will be valid for */
  sessionTtl: Scalars['Int']['output'];
  /** Whether to trust proxy headers for determining client IP and scheme (e.g., X-Forwarded-For) */
  trustProxyHeaders: Scalars['Boolean']['output'];
  /**
   * The verbosity with which system logs are visible (default: 1)
   * 0 = none at all, 1 = info, 2 = debug, 3 = trace
   */
  verbosity: Scalars['Int']['output'];
};

export type Subscription = {
  __typename?: 'Subscription';
  readEvents: CoreEvent;
  tailLogFile: Scalars['String']['output'];
};

export type SuggestBookInput = {
  author?: InputMaybe<Scalars['String']['input']>;
  bookId?: InputMaybe<Scalars['String']['input']>;
  notes?: InputMaybe<Scalars['String']['input']>;
  title?: InputMaybe<Scalars['String']['input']>;
  url?: InputMaybe<Scalars['String']['input']>;
};

export enum SupportedFont {
  AtkinsonHyperlegibleNext = 'ATKINSON_HYPERLEGIBLE_NEXT',
  Bitter = 'BITTER',
  Charis = 'CHARIS',
  HinaMincho = 'HINA_MINCHO',
  Inter = 'INTER',
  LibreBaskerville = 'LIBRE_BASKERVILLE',
  Literata = 'LITERATA',
  Nunito = 'NUNITO',
  OpenDyslexic = 'OPEN_DYSLEXIC'
}

/** Supported image formats for processing images throughout Stump */
export enum SupportedImageFormat {
  Jpeg = 'JPEG',
  Png = 'PNG',
  Webp = 'WEBP'
}

export enum SystemArrangement {
  BookClubs = 'BOOK_CLUBS',
  Explore = 'EXPLORE',
  Home = 'HOME',
  Libraries = 'LIBRARIES',
  SmartLists = 'SMART_LISTS'
}

export type SystemArrangementConfig = {
  __typename?: 'SystemArrangementConfig';
  links: Array<FilterableArrangementEntityLink>;
  variant: SystemArrangement;
};

export type SystemArrangementConfigInput = {
  links?: Array<FilterableArrangementEntityLink>;
  variant: SystemArrangement;
};

export type Tag = {
  __typename?: 'Tag';
  id: Scalars['Int']['output'];
  name: Scalars['String']['output'];
};

export type TelegramConfig = {
  __typename?: 'TelegramConfig';
  chatId: Scalars['String']['output'];
  encryptedToken: Scalars['String']['output'];
};

export type TelegramConfigInput = {
  chatId: Scalars['String']['input'];
  token: Scalars['String']['input'];
};

export type ThumbnailGenerationOutput = {
  __typename?: 'ThumbnailGenerationOutput';
  /** The number of thumbnails that were generated */
  generatedThumbnails: Scalars['Int']['output'];
  /** The number of thumbnails that were removed */
  removedThumbnails: Scalars['Int']['output'];
  /** The number of thumbnails that were skipped (already existed and not force regenerated) */
  skippedFiles: Scalars['Int']['output'];
  /** The total number of files that were visited during the thumbnail generation */
  visitedFiles: Scalars['Int']['output'];
};

/** The style of placeholder to use for thumbnails while they are loading */
export enum ThumbnailPlaceholderStyle {
  AverageColor = 'AVERAGE_COLOR',
  Colorful = 'COLORFUL',
  Grayscale = 'GRAYSCALE',
  Thumbhash = 'THUMBHASH'
}

export type TimeToFinishStat = {
  __typename?: 'TimeToFinishStat';
  libraryType: LibraryType;
  medianActiveSeconds: Scalars['Int']['output'];
  medianCalendarDays: Scalars['Float']['output'];
  medianSecondsPerPage?: Maybe<Scalars['Float']['output']>;
  sampleSize: Scalars['Int']['output'];
};

export type TrashPurgeOutput = {
  __typename?: 'TrashPurgeOutput';
  /** Why books were kept (the first few) */
  errors: Array<DeletedBooksPurgeError>;
  /** The disk space given back (deleted book files and held files) */
  freedBytes: Scalars['Int']['output'];
  /** Books kept in Deleted books because their file or data couldn't be deleted */
  keptBooks: Scalars['Int']['output'];
  /** Books whose file was already gone: only their data was deleted */
  missingBookFiles: Scalars['Int']['output'];
  /** Books deleted permanently (their file and their data) */
  purgedBooks: Scalars['Int']['output'];
  /** Held files (originals replaced by post-processing) deleted */
  purgedHeldFiles: Scalars['Int']['output'];
};

/**
 * A simple pagination input object which does not paginate. An explicit struct is
 * required as a limitation of async_graphql's [OneofObject], which doesn't allow
 * for empty variants.
 */
export type Unpaginated = {
  unpaginated: Scalars['Boolean']['input'];
};

/**
 * Changes an annotation's note and/or style. An omitted field is left unchanged, and a
 * null `annotationText` removes the note
 */
export type UpdateAnnotationInput = {
  annotationText?: InputMaybe<Scalars['String']['input']>;
  id: Scalars['String']['input'];
  style?: InputMaybe<AnnotationStyle>;
};

export type UpdateBookClubInput = {
  description?: InputMaybe<Scalars['String']['input']>;
  emoji?: InputMaybe<Scalars['String']['input']>;
  isPrivate?: InputMaybe<Scalars['Boolean']['input']>;
  memberRoleSpec?: InputMaybe<Scalars['JSON']['input']>;
  name?: InputMaybe<Scalars['String']['input']>;
};

export type UpdateCustomEmojiInput = {
  name: Scalars['String']['input'];
};

/**
 * What `updateReadingList` changes: the name and description replace the list's, the
 * visibility stays when omitted. The books are changed with `addBooksToReadingList` and
 * `removeBooksFromReadingList`
 */
export type UpdateReadingListInput = {
  /** Omitted or empty clears it */
  description?: InputMaybe<Scalars['String']['input']>;
  name: Scalars['String']['input'];
  visibility?: InputMaybe<EntityVisibility>;
};

export type UpdateScheduledJobInput = {
  /** Replace the config entirely. The kind is inferred from the variant */
  config?: InputMaybe<ScheduledJobConfigInput>;
  enabled?: InputMaybe<Scalars['Boolean']['input']>;
  name?: InputMaybe<Scalars['String']['input']>;
  /** A cron expression */
  schedule?: InputMaybe<Scalars['String']['input']>;
};

export type UpdateThumbnailInput = {
  /** A flag indicating whether the page is zero based (i.e. 0 is the first page) */
  isZeroBased?: InputMaybe<Scalars['Boolean']['input']>;
  /** The ID of the media inside the series to fetch */
  mediaId: Scalars['String']['input'];
  /** The page to pull inside the media file for generating the thumbnail */
  page: Scalars['Int']['input'];
};

export type UpdateUserInput = {
  ageRestriction?: InputMaybe<AgeRestrictionInput>;
  maxSessionsAllowed?: InputMaybe<Scalars['Int']['input']>;
  password?: InputMaybe<Scalars['String']['input']>;
  permissions: Array<UserPermission>;
  username: Scalars['String']['input'];
};

export type UpdateUserPreferencesInput = {
  appFont: SupportedFont;
  appTheme: Scalars['String']['input'];
  dayResetHourOffset: Scalars['Int']['input'];
  enableAlphabetSelect: Scalars['Boolean']['input'];
  enableDiscordPresence: Scalars['Boolean']['input'];
  enableFancyAnimations: Scalars['Boolean']['input'];
  enableGradients: Scalars['Boolean']['input'];
  enableHideScrollbar: Scalars['Boolean']['input'];
  enableJobOverlay: Scalars['Boolean']['input'];
  enableLiveRefetch: Scalars['Boolean']['input'];
  enableReadingJournal: Scalars['Boolean']['input'];
  interfaceRoundness: InterfaceRoundness;
  locale: Scalars['String']['input'];
  preferAccentColor: Scalars['Boolean']['input'];
  preferredLayoutMode: InterfaceLayout;
  readingSessionGracePeriodSecs: Scalars['Int']['input'];
  showQueryIndicator: Scalars['Boolean']['input'];
  thumbnailPlaceholderStyle: ThumbnailPlaceholderStyle;
  thumbnailRatio: Scalars['Float']['input'];
  thumbnailRoundness: InterfaceRoundness;
};

export type UploadBooksInput = {
  libraryId: Scalars['String']['input'];
  placeAt: Scalars['String']['input'];
  uploads: Array<Scalars['Upload']['input']>;
};

export type UploadConfig = {
  __typename?: 'UploadConfig';
  enabled: Scalars['Boolean']['output'];
  maxFileUploadSize: Scalars['Int']['output'];
};

export type UploadSeriesInput = {
  libraryId: Scalars['String']['input'];
  placeAt: Scalars['String']['input'];
  seriesDirName: Scalars['String']['input'];
  upload: Scalars['Upload']['input'];
};

export type User = {
  __typename?: 'User';
  ageRestriction?: Maybe<AgeRestriction>;
  /** a reference to the avatar image and its metadata for this user */
  avatar: ImageRef;
  avatarMeta?: Maybe<ImageMetadata>;
  avatarPath?: Maybe<Scalars['String']['output']>;
  avatarUpdatedAt?: Maybe<Scalars['DateTime']['output']>;
  /** @deprecated This will be deprecated in a future release which refactors the auth RESTful API. Until then, it stays. */
  avatarUrl?: Maybe<Scalars['String']['output']>;
  continueReading: PaginatedMediaResponse;
  createdAt: Scalars['DateTime']['output'];
  deletedAt?: Maybe<Scalars['DateTime']['output']>;
  finishedReadingSessionsCount: Scalars['Int']['output'];
  id: Scalars['String']['output'];
  isLocked: Scalars['Boolean']['output'];
  isServerOwner: Scalars['Boolean']['output'];
  lastLogin?: Maybe<Scalars['DateTime']['output']>;
  loginSessionsCount: Scalars['Int']['output'];
  maxSessionsAllowed?: Maybe<Scalars['Int']['output']>;
  oidcEmail?: Maybe<Scalars['String']['output']>;
  permissions: Array<UserPermission>;
  preferences: UserPreferences;
  username: Scalars['String']['output'];
};


export type UserContinueReadingArgs = {
  pagination?: Pagination;
};

export type UserLoginActivity = {
  __typename?: 'UserLoginActivity';
  authenticationSuccessful: Scalars['Boolean']['output'];
  id: Scalars['Int']['output'];
  ipAddress: Scalars['String']['output'];
  timestamp: Scalars['DateTime']['output'];
  user: User;
  userAgent: Scalars['String']['output'];
  userId: Scalars['String']['output'];
};

/** The permissions a user may be granted */
export enum UserPermission {
  /** Grant access to read/create their own API keys */
  AccessApiKeys = 'ACCESS_API_KEYS',
  /**
   * TODO: Expand permissions for bookclub + smartlist
   * Grant access to the book club feature
   */
  AccessBookClub = 'ACCESS_BOOK_CLUB',
  /** Grant access to the kobo sync feature */
  AccessKoboSync = 'ACCESS_KOBO_SYNC',
  /** Grant access to the koreader sync feature */
  AccessKoreaderSync = 'ACCESS_KOREADER_SYNC',
  /** Grant access to access the smart list feature. This includes the ability to create and edit smart lists */
  AccessSmartList = 'ACCESS_SMART_LIST',
  /**
   * Grant access to the "Add books" inbox: upload books, review the proposed metadata,
   * author folder and tags, and file them into the libraries the user can access
   */
  AddBooks = 'ADD_BOOKS',
  /** Grant user access to change **their own** avatar */
  ChangeAvatar = 'CHANGE_AVATAR',
  /** Grant user access to change **their own** password */
  ChangePassword = 'CHANGE_PASSWORD',
  /** Grant user access to change **their own** username */
  ChangeUsername = 'CHANGE_USERNAME',
  /** Grant access to create a book club (access book club) */
  CreateBookClub = 'CREATE_BOOK_CLUB',
  /** Grant access to create a library */
  CreateLibrary = 'CREATE_LIBRARY',
  /** Grant access to create a notifier */
  CreateNotifier = 'CREATE_NOTIFIER',
  /** Grant access to delete the library (manage library) */
  DeleteLibrary = 'DELETE_LIBRARY',
  /** Grant access to delete a notifier */
  DeleteNotifier = 'DELETE_NOTIFIER',
  /** Grant access to download files from a library */
  DownloadFile = 'DOWNLOAD_FILE',
  /** Grant access to edit basic details about the library */
  EditLibrary = 'EDIT_LIBRARY',
  /**
   * Grants access to edit any existing metadata for media/series. This will only
   * be applied to the database-level metadata.
   */
  EditMetadata = 'EDIT_METADATA',
  /** Grant access to edit thumbnails for media/series */
  EditThumbnails = 'EDIT_THUMBNAILS',
  /** Grant access to create an emailer */
  EmailerCreate = 'EMAILER_CREATE',
  /** Grant access to manage an emailer */
  EmailerManage = 'EMAILER_MANAGE',
  /** Grant access to read any emailers in the system */
  EmailerRead = 'EMAILER_READ',
  /** Grant access to send an arbitrary email, bypassing any registered device requirements */
  EmailArbitrarySend = 'EMAIL_ARBITRARY_SEND',
  /** Grant access to send an email */
  EmailSend = 'EMAIL_SEND',
  /** Grant access to access the file explorer */
  FileExplorer = 'FILE_EXPLORER',
  /** Grant access to manage jobs, like pausing, resuming, deleting, or cancelling them */
  ManageJobs = 'MANAGE_JOBS',
  /** Grant access to manage the library (scan,edit,manage relations) */
  ManageLibrary = 'MANAGE_LIBRARY',
  /** Grant access to manage a notifier */
  ManageNotifier = 'MANAGE_NOTIFIER',
  /** Grant access to manage the server. This is effectively a step below server owner */
  ManageServer = 'MANAGE_SERVER',
  /** Grant access to manage users (create,edit,delete) */
  ManageUsers = 'MANAGE_USERS',
  /** Grant access to manage metadata fetch statuses (accept matches, etc) */
  MetadataFetchRecordManage = 'METADATA_FETCH_RECORD_MANAGE',
  /** Grant access to read metadata fetch statuses */
  MetadataFetchRecordRead = 'METADATA_FETCH_RECORD_READ',
  /** Grant access to manage metadata provider configurations (create, update, delete) */
  MetadataProviderManage = 'METADATA_PROVIDER_MANAGE',
  /** Grant access to read metadata provider configurations */
  MetadataProviderRead = 'METADATA_PROVIDER_READ',
  /** Grant access to read jobs */
  ReadJobs = 'READ_JOBS',
  /** Grant access to read notifiers */
  ReadNotifier = 'READ_NOTIFIER',
  /** Grant access to read application-level logs, e.g. job logs */
  ReadPersistedLogs = 'READ_PERSISTED_LOGS',
  /** Grant access to read system logs */
  ReadSystemLogs = 'READ_SYSTEM_LOGS',
  /**
   * Grant access to read users.
   *
   * Note that this is explicitly for querying users via user-specific endpoints.
   * This would not affect relational queries, such as members in a common book club.
   */
  ReadUsers = 'READ_USERS',
  /**
   * Grant access to remove books from their library. Removed books are hidden everywhere
   * but their files stay on disk, and a library manager can restore them
   */
  RemoveBook = 'REMOVE_BOOK',
  /** Grant access to scan the library for new files */
  ScanLibrary = 'SCAN_LIBRARY',
  /** Grant access to upload files to a library */
  UploadFile = 'UPLOAD_FILE',
  /**
   * Grants access to write back the database-level metadata for media/series.
   * This should be treated with caution, as technically it would allow for
   * overwriting existing metadata at the file-level
   */
  WriteBackMetadata = 'WRITE_BACK_METADATA'
}

export type UserPermissionStruct = {
  __typename?: 'UserPermissionStruct';
  value: Array<UserPermission>;
};

export type UserPreferences = {
  __typename?: 'UserPreferences';
  appFont: SupportedFont;
  appTheme: Scalars['String']['output'];
  /**
   * hour offset from midnight at which a new "logical day" begins for reading sessions
   * 0 = midnight, 2 = 2am, etc
   */
  dayResetHourOffset: Scalars['Int']['output'];
  enableAlphabetSelect: Scalars['Boolean']['output'];
  enableDiscordPresence: Scalars['Boolean']['output'];
  enableFancyAnimations: Scalars['Boolean']['output'];
  enableGradients: Scalars['Boolean']['output'];
  enableHideScrollbar: Scalars['Boolean']['output'];
  enableJobOverlay: Scalars['Boolean']['output'];
  enableLiveRefetch: Scalars['Boolean']['output'];
  enableReadingJournal: Scalars['Boolean']['output'];
  homeArrangement: Arrangement;
  interfaceRoundness: InterfaceRoundness;
  locale: Scalars['String']['output'];
  navigationArrangement: Arrangement;
  preferAccentColor: Scalars['Boolean']['output'];
  preferredLayoutMode: InterfaceLayout;
  /** seconds of inactivity after which the current reading session is considered ended */
  readingSessionGracePeriodSecs: Scalars['Int']['output'];
  showQueryIndicator: Scalars['Boolean']['output'];
  thumbnailPlaceholderStyle: ThumbnailPlaceholderStyle;
  thumbnailRatio: Scalars['Float']['output'];
  thumbnailRoundness: InterfaceRoundness;
  userId?: Maybe<Scalars['String']['output']>;
};

export type ValidateMetadataProviderConfigInput = {
  /** The API token for authenticating with the provider */
  apiToken: Scalars['String']['input'];
  /** The provider type */
  providerType: MetadataProvider;
};

export type WeekdayActivity = {
  __typename?: 'WeekdayActivity';
  /** Total seconds divided by the number of times this weekday occurs in the range */
  averageSeconds: Scalars['Int']['output'];
  /** 1 (Monday) -> 7 (Sunday) */
  isoWeekday: Scalars['Int']['output'];
  totalSeconds: Scalars['Int']['output'];
};

export type CreateBookClubMobileMutationVariables = Exact<{
  input: CreateBookClubInput;
}>;


export type CreateBookClubMobileMutation = { __typename?: 'Mutation', createBookClub: { __typename?: 'BookClub', id: string, slug: string } };

export type BookClubsScreenQueryVariables = Exact<{ [key: string]: never; }>;


export type BookClubsScreenQuery = { __typename?: 'Query', bookClubs: Array<(
    { __typename?: 'BookClub', id: string }
    & { ' $fragmentRefs'?: { 'BookClubCardFragment': BookClubCardFragment } }
  )>, myBookClubInvitations: Array<{ __typename?: 'BookClubInvitation', id: string }> };

export type BookClubInvitesScreenQueryVariables = Exact<{ [key: string]: never; }>;


export type BookClubInvitesScreenQuery = { __typename?: 'Query', myBookClubInvitations: Array<{ __typename?: 'BookClubInvitation', id: string, role: BookClubMemberRole, bookClubId: string, bookClub: { __typename?: 'BookClub', name: string, description?: string | null, membersCount: number } }> };

export type RespondToBookClubInvitationMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  accept: Scalars['Boolean']['input'];
}>;


export type RespondToBookClubInvitationMutation = { __typename?: 'Mutation', respondToBookClubInvitation: { __typename?: 'BookClubInvitation', id: string } };

export type SearchMediaQueryVariables = Exact<{
  filter: MediaFilterInput;
}>;


export type SearchMediaQuery = { __typename?: 'Query', media: { __typename?: 'PaginatedMediaResponse', nodes: Array<(
      { __typename?: 'Media', id: string }
      & { ' $fragmentRefs'?: { 'BookSearchItemFragment': BookSearchItemFragment } }
    )>, pageInfo: { __typename: 'CursorPaginationInfo', nextCursor?: string | null } | { __typename: 'OffsetPaginationInfo' } } };

export type SearchSeriesQueryVariables = Exact<{
  filter: SeriesFilterInput;
}>;


export type SearchSeriesQuery = { __typename?: 'Query', series: { __typename?: 'PaginatedSeriesResponse', nodes: Array<(
      { __typename?: 'Series', id: string }
      & { ' $fragmentRefs'?: { 'SeriesSearchItemFragment': SeriesSearchItemFragment } }
    )>, pageInfo: { __typename: 'CursorPaginationInfo', nextCursor?: string | null } | { __typename: 'OffsetPaginationInfo' } } };

export type SearchLibraryQueryVariables = Exact<{
  search: Scalars['String']['input'];
}>;


export type SearchLibraryQuery = { __typename?: 'Query', libraries: { __typename?: 'PaginatedLibraryResponse', nodes: Array<(
      { __typename?: 'Library', id: string }
      & { ' $fragmentRefs'?: { 'LibrarySearchItemFragment': LibrarySearchItemFragment } }
    )>, pageInfo: { __typename: 'CursorPaginationInfo', nextCursor?: string | null } | { __typename: 'OffsetPaginationInfo' } } };

export type BookByIdStackLayoutQueryVariables = Exact<{
  bookId: Scalars['ID']['input'];
}>;


export type BookByIdStackLayoutQuery = { __typename?: 'Query', mediaById?: { __typename?: 'Media', id: string, thumbnail: { __typename?: 'ImageRef', metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null } } | null };

export type BookByIdQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type BookByIdQuery = { __typename?: 'Query', mediaById?: (
    { __typename?: 'Media', id: string, extension: string, pages: number, resolvedName: string, seriesPosition?: number | null, size: number, metadata?: { __typename?: 'MediaMetadata', ageRating?: number | null, characters: Array<string>, colorists: Array<string>, coverArtists: Array<string>, day?: number | null, editors: Array<string>, identifierAmazon?: string | null, identifierCalibre?: string | null, identifierGoogle?: string | null, identifierIsbn?: string | null, identifierMobiAsin?: string | null, identifierUuid?: string | null, genres: Array<string>, inkers: Array<string>, language?: string | null, letterers: Array<string>, links: Array<string>, month?: number | null, notes?: string | null, number?: any | null, pageCount?: number | null, pencillers: Array<string>, publisher?: string | null, series?: string | null, summary?: string | null, teams: Array<string>, title?: string | null, titleSort?: string | null, volume?: number | null, writers: Array<string>, year?: number | null } | null, readProgress?: { __typename?: 'ResumeReadingCursor', page?: number | null, percentageCompleted?: any | null, startedAt?: any | null, elapsedSeconds: number, updatedAt?: any | null, locator?: { __typename?: 'ReadiumLocator', chapterTitle: string, href: string, title?: string | null, type: string, locations?: { __typename?: 'ReadiumLocation', fragments?: Array<string> | null, position?: number | null, progression?: any | null, totalProgression?: any | null, cssSelector?: string | null, partialCfi?: string | null } | null } | null } | null, readHistory: Array<{ __typename?: 'ReadthroughRecord', completedAt: any, elapsedSeconds: number }>, series?: { __typename?: 'Series', id: string, resolvedName: string, mediaCount: number, metadata?: { __typename?: 'SeriesMetadata', totalIssues?: number | null } | null } | null, library: { __typename?: 'Library', id: string, name: string }, thumbnail: { __typename?: 'ImageRef', url: string, height?: number | null, width?: number | null, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null }, ebook?: { __typename?: 'Epub', toc: Array<string> } | null }
    & { ' $fragmentRefs'?: { 'BookMenuFragment': BookMenuFragment } }
  ) | null };

export type BookReadScreenQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type BookReadScreenQuery = { __typename?: 'Query', mediaById?: { __typename?: 'Media', id: string, pages: number, extension: string, name: string, thumbnail: { __typename?: 'ImageRef', url: string, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null }, readProgress?: { __typename?: 'ResumeReadingCursor', percentageCompleted?: any | null, page?: number | null, elapsedSeconds: number, locator?: { __typename?: 'ReadiumLocator', chapterTitle: string, href: string, title?: string | null, type: string, locations?: { __typename?: 'ReadiumLocation', fragments?: Array<string> | null, progression?: any | null, position?: number | null, totalProgression?: any | null, cssSelector?: string | null, partialCfi?: string | null } | null } | null } | null, series?: { __typename?: 'Series', id: string, resolvedName: string } | null, library: { __typename?: 'Library', id: string, name: string }, libraryConfig: { __typename?: 'LibraryConfig', defaultReadingImageScaleFit: ReadingImageScaleFit, defaultReadingMode: ReadingMode, defaultReadingDir: ReadingDirection }, metadata?: { __typename?: 'MediaMetadata', writers: Array<string>, publisher?: string | null, summary?: string | null } | null, analysisData?: { __typename?: 'MediaAnalysisData', dimensions: Array<{ __typename?: 'PageDimension', height: number, width: number }> } | null, nextInSeries: { __typename?: 'PaginatedMediaResponse', nodes: Array<{ __typename?: 'Media', id: string, name: string, thumbnail: { __typename?: 'ImageRef', url: string } }> }, ebook?: { __typename?: 'Epub', toc: Array<string>, bookmarks: Array<{ __typename?: 'Bookmark', id: string, mediaId: string, previewContent?: string | null, createdAt: any, locator?: { __typename?: 'ReadiumLocator', chapterTitle: string, href: string, locations?: { __typename?: 'ReadiumLocation', fragments?: Array<string> | null, progression?: any | null, position?: number | null, totalProgression?: any | null, cssSelector?: string | null, partialCfi?: string | null } | null } | null }>, annotations: Array<{ __typename?: 'MediaAnnotationModel', id: string, annotationText?: string | null, createdAt: any, updatedAt: any, locator: { __typename?: 'ReadiumLocator', chapterTitle: string, href: string, title?: string | null, type: string, locations?: { __typename?: 'ReadiumLocation', fragments?: Array<string> | null, progression?: any | null, position?: number | null, totalProgression?: any | null, cssSelector?: string | null, partialCfi?: string | null } | null, text?: { __typename?: 'ReadiumText', after?: string | null, before?: string | null, highlight?: string | null } | null } }>, spine: Array<{ __typename?: 'SpineItem', id?: string | null, idref: string, properties?: string | null, linear: boolean }> } | null } | null };

export type UpdateReadProgressionMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  input: MediaProgressInput;
}>;


export type UpdateReadProgressionMutation = { __typename?: 'Mutation', updateMediaProgress: { __typename?: 'ReadingSession', id: number, updatedAt?: any | null } };

export type ResetElapsedSecondsMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type ResetElapsedSecondsMutation = { __typename?: 'Mutation', resetElapsedSeconds: boolean };

export type CreateBookmarkMobileMutationVariables = Exact<{
  input: BookmarkInput;
}>;


export type CreateBookmarkMobileMutation = { __typename?: 'Mutation', createBookmark: { __typename?: 'Bookmark', id: string, previewContent?: string | null, mediaId: string, locator?: { __typename?: 'ReadiumLocator', chapterTitle: string, href: string, locations?: { __typename?: 'ReadiumLocation', fragments?: Array<string> | null, progression?: any | null, position?: number | null, totalProgression?: any | null, cssSelector?: string | null, partialCfi?: string | null } | null } | null } };

export type DeleteBookmarkMobileMutationVariables = Exact<{
  id: Scalars['String']['input'];
}>;


export type DeleteBookmarkMobileMutation = { __typename?: 'Mutation', deleteBookmark: { __typename?: 'Bookmark', id: string } };

export type CreateAnnotationMobileMutationVariables = Exact<{
  input: CreateAnnotationInput;
}>;


export type CreateAnnotationMobileMutation = { __typename?: 'Mutation', createAnnotation: { __typename?: 'MediaAnnotation', id: string, annotationText?: string | null, createdAt: any, updatedAt: any, locator: { __typename?: 'ReadiumLocator', chapterTitle: string, href: string, title?: string | null, type: string, locations?: { __typename?: 'ReadiumLocation', fragments?: Array<string> | null, progression?: any | null, position?: number | null, totalProgression?: any | null, cssSelector?: string | null, partialCfi?: string | null } | null, text?: { __typename?: 'ReadiumText', after?: string | null, before?: string | null, highlight?: string | null } | null } } };

export type UpdateAnnotationMobileMutationVariables = Exact<{
  input: UpdateAnnotationInput;
}>;


export type UpdateAnnotationMobileMutation = { __typename?: 'Mutation', updateAnnotation: { __typename?: 'MediaAnnotation', id: string, annotationText?: string | null, updatedAt: any } };

export type DeleteAnnotationMobileMutationVariables = Exact<{
  id: Scalars['String']['input'];
}>;


export type DeleteAnnotationMobileMutation = { __typename?: 'Mutation', deleteAnnotation: { __typename?: 'MediaAnnotation', id: string } };

export type BookReadingTimelineSessionIdScreenQueryVariables = Exact<{
  sessionId: Scalars['Int']['input'];
  eventOrder?: InputMaybe<OrderDirection>;
}>;


export type BookReadingTimelineSessionIdScreenQuery = { __typename?: 'Query', readingSessionById?: (
    { __typename?: 'ReadingSession', id: number, createdAt: any, updatedAt?: any | null, startPage?: number | null, endPage?: number | null, endPercentage?: any | null, elapsedSeconds?: number | null, endLocator?: { __typename?: 'ReadiumLocator', locations?: { __typename?: 'ReadiumLocation', position?: number | null } | null } | null, media?: { __typename?: 'Media', resolvedName: string, pages: number, thumbnail: { __typename?: 'ImageRef', url: string, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null } } | null }
    & { ' $fragmentRefs'?: { 'ReadingSessionMenuFragment': ReadingSessionMenuFragment;'SessionEventsTimelineFragment': SessionEventsTimelineFragment;'ReadingSessionDetailHeaderFragment': ReadingSessionDetailHeaderFragment;'SessionJournalFragment': SessionJournalFragment } }
  ) | null };

export type BookReadingTimelineScreenQueryVariables = Exact<{
  bookId: Scalars['ID']['input'];
}>;


export type BookReadingTimelineScreenQuery = { __typename?: 'Query', mediaById?: (
    { __typename?: 'Media', id: string }
    & { ' $fragmentRefs'?: { 'BookReadingTimelineFragment': BookReadingTimelineFragment } }
  ) | null };

export type BooksScreenQueryVariables = Exact<{
  pagination?: InputMaybe<Pagination>;
  filters?: InputMaybe<MediaFilterInput>;
  orderBy?: InputMaybe<Array<MediaOrderBy> | MediaOrderBy>;
}>;


export type BooksScreenQuery = { __typename?: 'Query', media: { __typename?: 'PaginatedMediaResponse', nodes: Array<(
      { __typename?: 'Media', id: string, thumbnail: { __typename?: 'ImageRef', metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null } | null } }
      & { ' $fragmentRefs'?: { 'BookListItemFragment': BookListItemFragment } }
    )>, pageInfo: { __typename: 'CursorPaginationInfo' } | { __typename: 'OffsetPaginationInfo', totalPages: number, currentPage: number, pageSize: number, pageOffset: number, zeroBased: boolean } } };

export type BooksScreenStatsQueryVariables = Exact<{ [key: string]: never; }>;


export type BooksScreenStatsQuery = { __typename?: 'Query', librariesStats: { __typename?: 'LibraryStats', seriesCount: number, bookCount: number, totalBytes: number, completedBooks: number, inProgressBooks: number, totalReadingTimeSeconds: number } };

export type BookSearchScreenQueryVariables = Exact<{
  filter: MediaFilterInput;
  pagination: Pagination;
}>;


export type BookSearchScreenQuery = { __typename?: 'Query', media: { __typename?: 'PaginatedMediaResponse', nodes: Array<(
      { __typename?: 'Media', id: string }
      & { ' $fragmentRefs'?: { 'BookListItemFragment': BookListItemFragment } }
    )>, pageInfo: { __typename: 'CursorPaginationInfo', currentCursor?: string | null, nextCursor?: string | null, limit: number } | { __typename: 'OffsetPaginationInfo' } } };

export type BookClubContextLayoutQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type BookClubContextLayoutQuery = { __typename?: 'Query', bookClubById: { __typename?: 'BookClub', id: string, membership?: { __typename?: 'BookClubMember', id: string, role: BookClubMemberRole } | null } };

export type BookClubPastDiscussionsQueryVariables = Exact<{
  bookClubId: Scalars['ID']['input'];
}>;


export type BookClubPastDiscussionsQuery = { __typename?: 'Query', previousBookClubDiscussions: Array<{ __typename?: 'BookClubDiscussion', displayName: string, createdAt: any, messageCount: number, book?: (
      { __typename?: 'BookClubBook', id: string }
      & { ' $fragmentRefs'?: { 'PastBookGridItemFragment': PastBookGridItemFragment } }
    ) | null }> };

export type BookClubPastBookScreenQueryVariables = Exact<{
  bookId: Scalars['ID']['input'];
}>;


export type BookClubPastBookScreenQuery = { __typename?: 'Query', bookClubDiscussionByBook: Array<(
    { __typename?: 'BookClubDiscussion', id: string }
    & { ' $fragmentRefs'?: { 'DiscussionListItemFragment': DiscussionListItemFragment } }
  )>, bookClubBook: { __typename?: 'BookClubBook', title?: string | null, entity?: { __typename?: 'Media', resolvedName: string } | null } };

export type BookClubDiscussionRoomQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type BookClubDiscussionRoomQuery = { __typename?: 'Query', bookClubDiscussion: { __typename?: 'BookClubDiscussion', id: string, displayName: string, isLocked: boolean, book?: { __typename?: 'BookClubBook', id: string, title?: string | null, author?: string | null } | null } };

export type BookClubDiscussionMessagesQueryVariables = Exact<{
  discussionId: Scalars['ID']['input'];
  pagination?: InputMaybe<CursorPagination>;
}>;


export type BookClubDiscussionMessagesQuery = { __typename?: 'Query', bookClubDiscussionMessages: { __typename?: 'CursorPaginatedBookClubDiscussionMessageResponse', nodes: Array<{ __typename?: 'BookClubDiscussionMessage', id: string, content: string, timestamp: any, editedAt?: any | null, deletedAt?: string | null, isPinnedMessage: boolean, parentMessageId?: string | null, memberId?: string | null, threadChildrenCount: number, reactions: Array<{ __typename?: 'AggregatedReaction', emoji?: string | null, customEmojiId?: number | null, customEmojiUrl?: string | null, count: number, reactedByMe: boolean }>, replyTo?: { __typename?: 'BookClubDiscussionMessage', id: string, content: string, member?: { __typename?: 'BookClubMember', displayName?: string | null, username: string, avatarUrl?: string | null } | null } | null, member?: { __typename?: 'BookClubMember', id: string, displayName?: string | null, avatarUrl?: string | null, username: string } | null }>, cursorInfo: { __typename?: 'CursorPaginationInfo', nextCursor?: string | null, limit: number } } };

export type SendDiscussionMessageMutationVariables = Exact<{
  discussionId: Scalars['ID']['input'];
  input: SendMessageInput;
}>;


export type SendDiscussionMessageMutation = { __typename?: 'Mutation', sendMessage: { __typename?: 'BookClubDiscussionMessage', id: string } };

export type ToggleMessageReactionMutationVariables = Exact<{
  messageId: Scalars['ID']['input'];
  emoji?: InputMaybe<Scalars['String']['input']>;
  customEmojiId?: InputMaybe<Scalars['Int']['input']>;
}>;


export type ToggleMessageReactionMutation = { __typename?: 'Mutation', toggleReaction: boolean };

export type DeleteDiscussionMessageMutationVariables = Exact<{
  messageId: Scalars['ID']['input'];
}>;


export type DeleteDiscussionMessageMutation = { __typename?: 'Mutation', deleteMessage: { __typename?: 'BookClubDiscussionMessage', id: string } };

export type ThreadParentMessageQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type ThreadParentMessageQuery = { __typename?: 'Query', bookClubDiscussionMessage: { __typename?: 'BookClubDiscussionMessage', id: string, content: string, timestamp: any, editedAt?: any | null, deletedAt?: string | null, isPinnedMessage: boolean, parentMessageId?: string | null, memberId?: string | null, threadChildrenCount: number, reactions: Array<{ __typename?: 'AggregatedReaction', emoji?: string | null, customEmojiId?: number | null, customEmojiUrl?: string | null, count: number, reactedByMe: boolean }>, replyTo?: { __typename?: 'BookClubDiscussionMessage', id: string, content: string, member?: { __typename?: 'BookClubMember', displayName?: string | null, username: string, avatarUrl?: string | null } | null } | null, member?: { __typename?: 'BookClubMember', id: string, displayName?: string | null, avatarUrl?: string | null, username: string } | null } };

export type ThreadRepliesQueryVariables = Exact<{
  discussionId: Scalars['ID']['input'];
  parentId?: InputMaybe<Scalars['ID']['input']>;
  pagination?: InputMaybe<CursorPagination>;
}>;


export type ThreadRepliesQuery = { __typename?: 'Query', bookClubDiscussionMessages: { __typename?: 'CursorPaginatedBookClubDiscussionMessageResponse', nodes: Array<{ __typename?: 'BookClubDiscussionMessage', id: string, content: string, timestamp: any, editedAt?: any | null, deletedAt?: string | null, isPinnedMessage: boolean, parentMessageId?: string | null, memberId?: string | null, threadChildrenCount: number, reactions: Array<{ __typename?: 'AggregatedReaction', emoji?: string | null, customEmojiId?: number | null, count: number, reactedByMe: boolean }>, replyTo?: { __typename?: 'BookClubDiscussionMessage', id: string, content: string, member?: { __typename?: 'BookClubMember', displayName?: string | null, username: string } | null } | null, member?: { __typename?: 'BookClubMember', id: string, displayName?: string | null, avatarUrl?: string | null, username: string } | null }>, cursorInfo: { __typename?: 'CursorPaginationInfo', nextCursor?: string | null, limit: number } } };

export type SendThreadReplyMutationVariables = Exact<{
  discussionId: Scalars['ID']['input'];
  input: SendMessageInput;
}>;


export type SendThreadReplyMutation = { __typename?: 'Mutation', sendMessage: { __typename?: 'BookClubDiscussionMessage', id: string } };

export type ToggleThreadMessageReactionMutationVariables = Exact<{
  messageId: Scalars['ID']['input'];
  emoji?: InputMaybe<Scalars['String']['input']>;
  customEmojiId?: InputMaybe<Scalars['Int']['input']>;
}>;


export type ToggleThreadMessageReactionMutation = { __typename?: 'Mutation', toggleReaction: boolean };

export type DeleteThreadMessageMutationVariables = Exact<{
  messageId: Scalars['ID']['input'];
}>;


export type DeleteThreadMessageMutation = { __typename?: 'Mutation', deleteMessage: { __typename?: 'BookClubDiscussionMessage', id: string } };

export type ThreadDiscussionInfoQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type ThreadDiscussionInfoQuery = { __typename?: 'Query', bookClubDiscussion: { __typename?: 'BookClubDiscussion', id: string, displayName: string, isLocked: boolean } };

export type BookClubDetailScreenQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type BookClubDetailScreenQuery = { __typename?: 'Query', bookClubById: (
    { __typename?: 'BookClub', id: string, name: string, emoji?: string | null, membership?: { __typename?: 'BookClubMember', id: string, role: BookClubMemberRole } | null, moderators: Array<{ __typename?: 'BookClubMember', id: string, avatarUrl?: string | null, displayName?: string | null }>, pinnedDiscussions: Array<(
      { __typename?: 'BookClubDiscussion', id: string }
      & { ' $fragmentRefs'?: { 'DiscussionListItemFragment': DiscussionListItemFragment } }
    )>, currentBook?: (
      { __typename?: 'BookClubBook', id: string, discussions: Array<(
        { __typename?: 'BookClubDiscussion', id: string }
        & { ' $fragmentRefs'?: { 'DiscussionListItemFragment': DiscussionListItemFragment } }
      )>, entity?: { __typename?: 'Media', id: string, readProgress?: { __typename?: 'ResumeReadingCursor', percentageCompleted?: any | null, elapsedSeconds: number, startedAt?: any | null } | null, readHistory: Array<{ __typename: 'ReadthroughRecord', completedAt: any }> } | null }
      & { ' $fragmentRefs'?: { 'CurrentBookCardFragment': CurrentBookCardFragment } }
    ) | null }
    & { ' $fragmentRefs'?: { 'PastDiscussionsLinkFragment': PastDiscussionsLinkFragment } }
  ) };

export type BookClubSettingsQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type BookClubSettingsQuery = { __typename?: 'Query', bookClubById: { __typename?: 'BookClub', id: string, name: string, description?: string | null, isPrivate: boolean, emoji?: string | null, membership?: { __typename?: 'BookClubMember', id: string, role: BookClubMemberRole } | null } };

export type UpdateBookClubSettingsMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  input: UpdateBookClubInput;
}>;


export type UpdateBookClubSettingsMutation = { __typename?: 'Mutation', updateBookClub: { __typename?: 'BookClub', id: string } };

export type DeleteBookClubMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type DeleteBookClubMutation = { __typename?: 'Mutation', deleteBookClub: { __typename?: 'BookClub', id: string } };

export type LeaveBookClubMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type LeaveBookClubMutation = { __typename?: 'Mutation', leaveBookClub: { __typename?: 'BookClubMember', id: string } };

export type LibraryPathsQueryVariables = Exact<{ [key: string]: never; }>;


export type LibraryPathsQuery = { __typename?: 'Query', libraries: { __typename?: 'PaginatedLibraryResponse', nodes: Array<{ __typename?: 'Library', id: string, name: string, path: string }> } };

export type LibrarySeriesScreenSeriesNameQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type LibrarySeriesScreenSeriesNameQuery = { __typename?: 'Query', libraryById?: { __typename?: 'Library', name: string, stats: { __typename?: 'LibraryStats', bookCount: number, seriesCount: number, completedBooks: number, inProgressBooks: number, totalReadingTimeSeconds: number } } | null };

export type LibrarySeriesScreenQueryVariables = Exact<{
  filter: SeriesFilterInput;
  orderBy?: InputMaybe<Array<SeriesOrderBy> | SeriesOrderBy>;
  pagination?: InputMaybe<Pagination>;
}>;


export type LibrarySeriesScreenQuery = { __typename?: 'Query', series: { __typename?: 'PaginatedSeriesResponse', nodes: Array<(
      { __typename?: 'Series', id: string, thumbnail: { __typename?: 'ImageRef', metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null } | null } }
      & { ' $fragmentRefs'?: { 'SeriesListItemFragment': SeriesListItemFragment } }
    )>, pageInfo: { __typename: 'CursorPaginationInfo', currentCursor?: string | null, nextCursor?: string | null, limit: number } | { __typename: 'OffsetPaginationInfo' } } };

export type LibrariesScreenQueryVariables = Exact<{
  pagination?: InputMaybe<Pagination>;
}>;


export type LibrariesScreenQuery = { __typename?: 'Query', libraries: { __typename?: 'PaginatedLibraryResponse', nodes: Array<(
      { __typename?: 'Library', id: string }
      & { ' $fragmentRefs'?: { 'LibraryGridItemFragment': LibraryGridItemFragment } }
    )>, pageInfo: { __typename: 'CursorPaginationInfo', currentCursor?: string | null, nextCursor?: string | null, limit: number } | { __typename: 'OffsetPaginationInfo' } } };

export type SeriesBooksSceneSeriesNameQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type SeriesBooksSceneSeriesNameQuery = { __typename?: 'Query', seriesById?: { __typename?: 'Series', kind: SeriesKind, resolvedName: string, libraryId?: string | null, stats: { __typename?: 'SeriesStats', bookCount: number, completedBooks: number, inProgressBooks: number, totalReadingTimeSeconds: number } } | null };

export type SeriesBooksScreenQueryVariables = Exact<{
  filter: MediaFilterInput;
  pagination?: InputMaybe<Pagination>;
  orderBy?: InputMaybe<Array<MediaOrderBy> | MediaOrderBy>;
}>;


export type SeriesBooksScreenQuery = { __typename?: 'Query', media: { __typename?: 'PaginatedMediaResponse', nodes: Array<(
      { __typename?: 'Media', id: string, thumbnail: { __typename?: 'ImageRef', metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null } | null } }
      & { ' $fragmentRefs'?: { 'BookListItemFragment': BookListItemFragment } }
    )>, pageInfo: { __typename: 'CursorPaginationInfo' } | { __typename: 'OffsetPaginationInfo', totalPages: number, currentPage: number, pageSize: number, pageOffset: number, zeroBased: boolean } } };

export type SeriesScreenQueryVariables = Exact<{
  pagination?: InputMaybe<Pagination>;
  filters?: InputMaybe<SeriesFilterInput>;
  orderBy?: InputMaybe<Array<SeriesOrderBy> | SeriesOrderBy>;
}>;


export type SeriesScreenQuery = { __typename?: 'Query', series: { __typename?: 'PaginatedSeriesResponse', nodes: Array<(
      { __typename?: 'Series', id: string, thumbnail: { __typename?: 'ImageRef', metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null } | null } }
      & { ' $fragmentRefs'?: { 'SeriesListItemFragment': SeriesListItemFragment } }
    )>, pageInfo: { __typename: 'CursorPaginationInfo' } | { __typename: 'OffsetPaginationInfo', totalPages: number, currentPage: number, pageSize: number, pageOffset: number, zeroBased: boolean } } };

export type SeriesScreenStatsQueryVariables = Exact<{ [key: string]: never; }>;


export type SeriesScreenStatsQuery = { __typename?: 'Query', librariesStats: { __typename?: 'LibraryStats', seriesCount: number, bookCount: number, totalBytes: number, completedBooks: number, inProgressBooks: number, totalReadingTimeSeconds: number } };

export type SmartListScreenQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type SmartListScreenQuery = { __typename?: 'Query', smartListById?: { __typename?: 'SmartList', id: string, name: string, description?: string | null, items: { __typename: 'SmartListGrouped', items: Array<{ __typename?: 'SmartListGroupedItem', entity?: { __typename: 'Library', id: string, name: string } | { __typename: 'Series', id: string, resolvedName: string } | null, books: Array<(
          { __typename?: 'Media', id: string, thumbnail: { __typename?: 'ImageRef', url: string, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null } }
          & { ' $fragmentRefs'?: { 'SmartListBookItemFragment': SmartListBookItemFragment } }
        )> }> } | { __typename: 'SmartListUngrouped', books: Array<(
        { __typename?: 'Media', id: string, thumbnail: { __typename?: 'ImageRef', url: string, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null } }
        & { ' $fragmentRefs'?: { 'SmartListBookItemFragment': SmartListBookItemFragment } }
      )> } } | null };

export type SmartListsTabListQueryVariables = Exact<{ [key: string]: never; }>;


export type SmartListsTabListQuery = { __typename?: 'Query', smartLists: Array<(
    { __typename?: 'SmartList', id: string }
    & { ' $fragmentRefs'?: { 'SmartListGridItemFragment': SmartListGridItemFragment } }
  )> };

export type PullServerAnnotationsQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type PullServerAnnotationsQuery = { __typename?: 'Query', annotationsByMediaId: Array<{ __typename?: 'MediaAnnotation', id: string, annotationText?: string | null, createdAt: any, updatedAt: any, locator: { __typename?: 'ReadiumLocator', chapterTitle: string, href: string, title?: string | null, type: string, locations?: { __typename?: 'ReadiumLocation', fragments?: Array<string> | null, progression?: any | null, position?: number | null, totalProgression?: any | null, cssSelector?: string | null, partialCfi?: string | null } | null, text?: { __typename?: 'ReadiumText', after?: string | null, before?: string | null, highlight?: string | null } | null } }> };

export type PullServerBookmarksQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type PullServerBookmarksQuery = { __typename?: 'Query', bookmarksByMediaId: Array<{ __typename?: 'Bookmark', id: string, mediaId: string, previewContent?: string | null, locator?: { __typename?: 'ReadiumLocator', chapterTitle: string, href: string, locations?: { __typename?: 'ReadiumLocation', fragments?: Array<string> | null, progression?: any | null, position?: number | null, totalProgression?: any | null, cssSelector?: string | null, partialCfi?: string | null } | null } | null }> };

export type PullServerAvatarQueryVariables = Exact<{ [key: string]: never; }>;


export type PullServerAvatarQuery = { __typename?: 'Query', me: { __typename?: 'User', avatar: { __typename?: 'ImageRef', url: string, lastModified?: any | null, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null } | null } } };

export type PullServerReadProgressionQueryVariables = Exact<{
  filter: MediaFilterInput;
}>;


export type PullServerReadProgressionQuery = { __typename?: 'Query', media: { __typename?: 'PaginatedMediaResponse', nodes: Array<{ __typename?: 'Media', id: string, readProgress?: { __typename?: 'ResumeReadingCursor', sessionId: number, page?: number | null, percentageCompleted?: any | null, updatedAt?: any | null, elapsedSeconds: number, locator?: { __typename?: 'ReadiumLocator', chapterTitle: string, href: string, title?: string | null, type: string, locations?: { __typename?: 'ReadiumLocation', fragments?: Array<string> | null, progression?: any | null, position?: number | null, totalProgression?: any | null, cssSelector?: string | null, partialCfi?: string | null } | null } | null } | null, readHistory: Array<{ __typename?: 'ReadthroughRecord', completedAt: any }> }> } };

export type PushCreateAnnotationMutationVariables = Exact<{
  input: CreateAnnotationInput;
}>;


export type PushCreateAnnotationMutation = { __typename?: 'Mutation', createAnnotation: { __typename?: 'MediaAnnotation', id: string } };

export type PushUpdateAnnotationMutationVariables = Exact<{
  input: UpdateAnnotationInput;
}>;


export type PushUpdateAnnotationMutation = { __typename?: 'Mutation', updateAnnotation: { __typename?: 'MediaAnnotation', id: string } };

export type PushDeleteAnnotationMutationVariables = Exact<{
  id: Scalars['String']['input'];
}>;


export type PushDeleteAnnotationMutation = { __typename?: 'Mutation', deleteAnnotation: { __typename?: 'MediaAnnotation', id: string } };

export type PushCreateBookmarkMutationVariables = Exact<{
  input: BookmarkInput;
}>;


export type PushCreateBookmarkMutation = { __typename?: 'Mutation', createBookmark: { __typename?: 'Bookmark', id: string } };

export type PushDeleteBookmarkMutationVariables = Exact<{
  id: Scalars['String']['input'];
}>;


export type PushDeleteBookmarkMutation = { __typename?: 'Mutation', deleteBookmark: { __typename?: 'Bookmark', id: string } };

export type PushLocalReadProgressionMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  input: MediaProgressInput;
}>;


export type PushLocalReadProgressionMutation = { __typename?: 'Mutation', updateMediaProgress: { __typename?: 'ReadingSession', id: number, updatedAt?: any | null } };

export type ContinueReadingQueryVariables = Exact<{
  pagination?: InputMaybe<Pagination>;
}>;


export type ContinueReadingQuery = { __typename?: 'Query', keepReading: { __typename?: 'PaginatedMediaResponse', nodes: Array<(
      { __typename?: 'Media', id: string }
      & { ' $fragmentRefs'?: { 'HorizontalBookListItemFragment': HorizontalBookListItemFragment;'ReadingNowFragment': ReadingNowFragment } }
    )>, pageInfo: { __typename: 'CursorPaginationInfo' } | { __typename: 'OffsetPaginationInfo', totalPages: number, currentPage: number, pageSize: number, pageOffset: number, zeroBased: boolean } } };

export type OnDeckBooksQueryVariables = Exact<{
  pagination?: InputMaybe<Pagination>;
}>;


export type OnDeckBooksQuery = { __typename?: 'Query', onDeck: { __typename?: 'PaginatedMediaResponse', nodes: Array<(
      { __typename?: 'Media', id: string }
      & { ' $fragmentRefs'?: { 'OnDeckBookItemFragment': OnDeckBookItemFragment } }
    )>, pageInfo: { __typename: 'CursorPaginationInfo' } | { __typename: 'OffsetPaginationInfo', totalPages: number, currentPage: number, pageSize: number, pageOffset: number, zeroBased: boolean } } };

export type ReadingNowFragment = { __typename?: 'Media', id: string, resolvedName: string, pages: number, metadata?: { __typename?: 'MediaMetadata', summary?: string | null, genres: Array<string>, links: Array<string>, publisher?: string | null, year?: number | null } | null, thumbnail: { __typename?: 'ImageRef', url: string, height?: number | null, width?: number | null, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null }, readProgress?: { __typename?: 'ResumeReadingCursor', page?: number | null, percentageCompleted?: any | null, updatedAt?: any | null, locator?: { __typename?: 'ReadiumLocator', locations?: { __typename?: 'ReadiumLocation', position?: number | null } | null } | null } | null } & { ' $fragmentName'?: 'ReadingNowFragment' };

export type RecentlyAddedBooksQueryVariables = Exact<{
  pagination?: InputMaybe<Pagination>;
}>;


export type RecentlyAddedBooksQuery = { __typename?: 'Query', recentlyAddedMedia: { __typename?: 'PaginatedMediaResponse', nodes: Array<(
      { __typename?: 'Media', id: string }
      & { ' $fragmentRefs'?: { 'HorizontalBookListItemFragment': HorizontalBookListItemFragment } }
    )>, pageInfo: { __typename: 'CursorPaginationInfo', currentCursor?: string | null, nextCursor?: string | null, limit: number } | { __typename: 'OffsetPaginationInfo' } } };

export type RecentlyAddedSeriesHorizontalQueryVariables = Exact<{
  pagination?: InputMaybe<Pagination>;
}>;


export type RecentlyAddedSeriesHorizontalQuery = { __typename?: 'Query', recentlyAddedSeries: { __typename?: 'PaginatedSeriesResponse', nodes: Array<(
      { __typename?: 'Series', id: string }
      & { ' $fragmentRefs'?: { 'RecentlyAddedSeriesItemFragment': RecentlyAddedSeriesItemFragment } }
    )>, pageInfo: { __typename: 'CursorPaginationInfo', currentCursor?: string | null, nextCursor?: string | null, limit: number } | { __typename: 'OffsetPaginationInfo' } } };

export type BookListItemFragment = { __typename?: 'Media', id: string, resolvedName: string, pages: number, thumbnail: { __typename?: 'ImageRef', url: string, height?: number | null, width?: number | null, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null }, readProgress?: { __typename?: 'ResumeReadingCursor', page?: number | null, percentageCompleted?: any | null } | null, readHistory: Array<{ __typename?: 'ReadthroughRecord', completedAt: any }> } & { ' $fragmentName'?: 'BookListItemFragment' };

export type BookSearchItemFragment = { __typename?: 'Media', id: string, resolvedName: string, size: number, pages: number, thumbnail: { __typename?: 'ImageRef', url: string, height?: number | null, width?: number | null, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null } } & { ' $fragmentName'?: 'BookSearchItemFragment' };

export type BooksAfterCursorQueryVariables = Exact<{
  id: Scalars['ID']['input'];
  pagination?: InputMaybe<Pagination>;
}>;


export type BooksAfterCursorQuery = { __typename?: 'Query', mediaById?: { __typename?: 'Media', nextInSeries: { __typename?: 'PaginatedMediaResponse', nodes: Array<(
        { __typename?: 'Media', id: string }
        & { ' $fragmentRefs'?: { 'HorizontalBookListItemFragment': HorizontalBookListItemFragment } }
      )>, pageInfo: { __typename: 'CursorPaginationInfo', currentCursor?: string | null, nextCursor?: string | null, limit: number } | { __typename: 'OffsetPaginationInfo' } } } | null };

export type HorizontalBookListItemFragment = { __typename?: 'Media', id: string, resolvedName: string, thumbnail: { __typename?: 'ImageRef', url: string, height?: number | null, width?: number | null, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null } } & { ' $fragmentName'?: 'HorizontalBookListItemFragment' };

export type OnDeckBookItemFragment = { __typename?: 'Media', id: string, resolvedName: string, seriesPosition?: number | null, metadata?: { __typename?: 'MediaMetadata', number?: any | null } | null, thumbnail: { __typename?: 'ImageRef', url: string, height?: number | null, width?: number | null, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null }, series?: { __typename?: 'Series', resolvedName: string, mediaCount: number, metadata?: { __typename?: 'SeriesMetadata', totalIssues?: number | null } | null } | null } & { ' $fragmentName'?: 'OnDeckBookItemFragment' };

export type SeriesBooksListHeaderScanSeriesMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type SeriesBooksListHeaderScanSeriesMutation = { __typename?: 'Mutation', scanSeries: boolean };

export type BookMenuFragment = { __typename?: 'Media', id: string, resolvedName: string, isFavorite: boolean, library: { __typename?: 'Library', id: string, name: string }, series?: { __typename?: 'Series', id: string, resolvedName: string } | null, readProgress?: { __typename: 'ResumeReadingCursor' } | null, readHistory: Array<{ __typename: 'ReadthroughRecord' }> } & { ' $fragmentName'?: 'BookMenuFragment' };

export type BookMenuCompleteMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type BookMenuCompleteMutation = { __typename?: 'Mutation', finishMediaProgress: boolean };

export type BookMenuDeleteSessionMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type BookMenuDeleteSessionMutation = { __typename?: 'Mutation', clearMediaProgress: boolean };

export type BookMenuDeleteHistoryMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type BookMenuDeleteHistoryMutation = { __typename?: 'Mutation', deleteMediaReadingHistory: number };

export type AddBookSheetQueryVariables = Exact<{
  pagination?: InputMaybe<Pagination>;
  filters?: InputMaybe<MediaFilterInput>;
}>;


export type AddBookSheetQuery = { __typename?: 'Query', media: { __typename?: 'PaginatedMediaResponse', nodes: Array<(
      { __typename?: 'Media', id: string }
      & { ' $fragmentRefs'?: { 'BookListItemFragment': BookListItemFragment } }
    )>, pageInfo: { __typename: 'CursorPaginationInfo' } | { __typename: 'OffsetPaginationInfo', totalPages: number, currentPage: number, pageSize: number, pageOffset: number, zeroBased: boolean } } };

export type BookClubCardFragment = { __typename?: 'BookClub', id: string, name: string, slug: string, description?: string | null, membersCount: number, members: Array<{ __typename?: 'BookClubMember', id: string, displayName?: string | null, avatarUrl?: string | null }>, currentBook?: { __typename?: 'BookClubBook', id: string, imageUrl?: string | null, title?: string | null, entity?: { __typename: 'Media', id: string, thumbnail: { __typename?: 'ImageRef', url: string, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null } } | null } | null } & { ' $fragmentName'?: 'BookClubCardFragment' };

export type CurrentBookCardFragment = { __typename?: 'BookClubBook', id: string, title?: string | null, author?: string | null, imageUrl?: string | null, addedAt: any, url?: string | null, entity?: { __typename: 'Media', id: string, resolvedName: string, thumbnail: { __typename?: 'ImageRef', url: string, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null } } | null } & { ' $fragmentName'?: 'CurrentBookCardFragment' };

export type AddBookToClubMutationVariables = Exact<{
  bookClubId: Scalars['ID']['input'];
  input: AddBookToClubInput;
}>;


export type AddBookToClubMutation = { __typename?: 'Mutation', addBookToClub: { __typename?: 'BookClub', id: string } };

export type ArchiveCurrentBookMutationVariables = Exact<{
  bookClubBookId: Scalars['ID']['input'];
}>;


export type ArchiveCurrentBookMutation = { __typename?: 'Mutation', completeBook: { __typename?: 'BookClub', id: string } };

export type PastBookGridItemFragment = { __typename?: 'BookClubBook', id: string, imageUrl?: string | null, title?: string | null, completedAt?: any | null, entity?: { __typename: 'Media', id: string, resolvedName: string, thumbnail: { __typename?: 'ImageRef', url: string, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null } } | null } & { ' $fragmentName'?: 'PastBookGridItemFragment' };

export type PastDiscussionsLinkFragment = { __typename?: 'BookClub', previousDiscussionsCount: number, previousBook?: { __typename?: 'BookClubBook', imageUrl?: string | null, entity?: { __typename: 'Media', id: string, thumbnail: { __typename?: 'ImageRef', url: string, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null } } | null } | null } & { ' $fragmentName'?: 'PastDiscussionsLinkFragment' };

export type PreviewBookSheetQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type PreviewBookSheetQuery = { __typename?: 'Query', mediaById?: { __typename?: 'Media', id: string, resolvedName: string, thumbnail: { __typename?: 'ImageRef', url: string, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null }, metadata?: { __typename?: 'MediaMetadata', genres: Array<string>, writers: Array<string> } | null, tags: Array<{ __typename?: 'Tag', name: string }> } | null };

export type SuggestionsPickerSheetQueryVariables = Exact<{
  bookClubId: Scalars['ID']['input'];
  status?: InputMaybe<BookClubSuggestionStatus>;
}>;


export type SuggestionsPickerSheetQuery = { __typename?: 'Query', bookClubSuggestions: Array<{ __typename?: 'BookClubBookSuggestion', id: string, title?: string | null, author?: string | null, url?: string | null, bookId?: string | null, notes?: string | null, suggestedBy: { __typename?: 'BookClubMember', user: { __typename?: 'User', username: string } } }> };

export type DiscussionListItemFragment = { __typename?: 'BookClubDiscussion', id: string, displayName: string, emoji?: string | null, messageCount: number, isLocked: boolean, isArchived: boolean } & { ' $fragmentName'?: 'DiscussionListItemFragment' };

export type UseEmojisQueryVariables = Exact<{ [key: string]: never; }>;


export type UseEmojisQuery = { __typename?: 'Query', customEmojis: Array<{ __typename?: 'CustomEmoji', id: number, name: string, isAnimated: boolean, url: string }> };

export type LibraryGridItemFragment = { __typename?: 'Library', id: string, name: string, series: Array<{ __typename?: 'Series', thumbnail: { __typename?: 'ImageRef', url: string, height?: number | null, width?: number | null, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null } }> } & { ' $fragmentName'?: 'LibraryGridItemFragment' };

export type LibraryOverviewSheetQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type LibraryOverviewSheetQuery = { __typename?: 'Query', libraryById?: { __typename?: 'Library', name: string, description?: string | null, stats: { __typename?: 'LibraryStats', seriesCount: number, bookCount: number, totalBytes: number, completedBooks: number, inProgressBooks: number, totalReadingTimeSeconds: number }, tags: Array<{ __typename?: 'Tag', name: string }> } | null };

export type LibrarySearchItemFragment = { __typename?: 'Library', id: string, name: string, thumbnail: { __typename?: 'ImageRef', url: string, height?: number | null, width?: number | null, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null } } & { ' $fragmentName'?: 'LibrarySearchItemFragment' };

export type LibrarySeriesListHeaderScanLibraryMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type LibrarySeriesListHeaderScanLibraryMutation = { __typename?: 'Mutation', scanLibrary: boolean };

export type ReadingSessionConflictViewQueryVariables = Exact<{
  mediaId: Scalars['ID']['input'];
  branchedSessionId?: InputMaybe<Scalars['Int']['input']>;
}>;


export type ReadingSessionConflictViewQuery = { __typename?: 'Query', readingSessionConflictView: { __typename?: 'ReadingSessionConflictResolutionView', ancestorSession?: { __typename: 'ReadingSession', id: number, endPage?: number | null, endPercentage?: any | null, elapsedSeconds?: number | null, createdAt: any, updatedAt?: any | null, readthroughNumber: number, endLocator?: { __typename?: 'ReadiumLocator', href: string, chapterTitle: string, locations?: { __typename?: 'ReadiumLocation', progression?: any | null, totalProgression?: any | null } | null } | null } | null, remoteSessions: Array<{ __typename: 'ReadingSession', id: number, endPage?: number | null, endPercentage?: any | null, elapsedSeconds?: number | null, createdAt: any, updatedAt?: any | null, readthroughNumber: number, endLocator?: { __typename?: 'ReadiumLocator', href: string, chapterTitle: string, locations?: { __typename?: 'ReadiumLocation', progression?: any | null, totalProgression?: any | null } | null } | null }> } };

export type AcceptLocalProgressMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  ancestorSessionId?: InputMaybe<Scalars['Int']['input']>;
  input: MediaProgressInput;
}>;


export type AcceptLocalProgressMutation = { __typename?: 'Mutation', acceptLocalProgress: { __typename?: 'ReadingSession', id: number, endPage?: number | null, endPercentage?: any | null, elapsedSeconds?: number | null, updatedAt?: any | null, endLocator?: { __typename?: 'ReadiumLocator', href: string, chapterTitle: string, locations?: { __typename?: 'ReadiumLocation', progression?: any | null, totalProgression?: any | null } | null } | null } };

export type MyReadingStreaksQueryVariables = Exact<{ [key: string]: never; }>;


export type MyReadingStreaksQuery = { __typename?: 'Query', myReadingStreaks: { __typename?: 'ReadingStreaks', currentDays: number, longestDays: number } };

export type DailyActivityGridFragment = { __typename?: 'ReadingStats', range: { __typename?: 'ResolvedDateRange', start?: any | null, end: any }, dailyActivity: Array<{ __typename?: 'DailyActivity', date: any, totalSeconds: number, sessionCount: number }> } & { ' $fragmentName'?: 'DailyActivityGridFragment' };

export type RangedUserReadingStatsQueryVariables = Exact<{
  range?: InputMaybe<DateRangeInput>;
}>;


export type RangedUserReadingStatsQuery = { __typename?: 'Query', myReadingStats: (
    { __typename?: 'ReadingStats', range: { __typename?: 'ResolvedDateRange', start?: any | null, end: any }, totals: { __typename?: 'ReadingTotals', totalSeconds: number, sessionCount: number, finishedCount: number, activeDayCount: number } }
    & { ' $fragmentRefs'?: { 'DailyActivityGridFragment': DailyActivityGridFragment;'WeekdayChartFragment': WeekdayChartFragment;'TimeToFinishStatsFragment': TimeToFinishStatsFragment } }
  ) };

export type TimeToFinishStatsFragment = { __typename?: 'ReadingStats', timeToFinish: Array<{ __typename?: 'TimeToFinishStat', libraryType: LibraryType, sampleSize: number, medianActiveSeconds: number }> } & { ' $fragmentName'?: 'TimeToFinishStatsFragment' };

export type WeekdayChartFragment = { __typename?: 'ReadingStats', dayOfWeek: Array<{ __typename?: 'WeekdayActivity', isoWeekday: number, averageSeconds: number }> } & { ' $fragmentName'?: 'WeekdayChartFragment' };

export type ReadingSessionCardFragment = (
  { __typename?: 'SessionWithEvents', session: { __typename?: 'ReadingSession', id: number, status: ReadingStatus, createdAt: any, updatedAt?: any | null, startPage?: number | null, endPage?: number | null, endPercentage?: any | null, elapsedSeconds?: number | null, mediaId: string, isLatest: boolean, endLocator?: { __typename?: 'ReadiumLocator', locations?: { __typename?: 'ReadiumLocation', position?: number | null } | null } | null, journalEntry?: { __typename?: 'JournalEntryModel', content: string } | null }, events: Array<{ __typename: 'Bookmark', id: string } | { __typename: 'MediaAnnotation', id: string, annotationText?: string | null }> }
  & { ' $fragmentRefs'?: { 'SessionWithEventsTimelineFragment': SessionWithEventsTimelineFragment } }
) & { ' $fragmentName'?: 'ReadingSessionCardFragment' };

export type ReadingSessionCardMediaFragment = { __typename?: 'SessionWithEvents', session: { __typename?: 'ReadingSession', media?: { __typename?: 'Media', resolvedName: string, pages: number, thumbnail: { __typename?: 'ImageRef', url: string, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null } } | null } } & { ' $fragmentName'?: 'ReadingSessionCardMediaFragment' };

export type PatchReadingSessionEditSheetMutationVariables = Exact<{
  sessionId: Scalars['Int']['input'];
  input: PatchReadingSession;
}>;


export type PatchReadingSessionEditSheetMutation = { __typename?: 'Mutation', patchReadingSession: { __typename?: 'ReadingSession', id: number, elapsedSeconds?: number | null } };

export type BookReadingTimelineFragment = { __typename?: 'Media', id: string, resolvedName: string, pages: number, readingTimeline?: { __typename?: 'BookReadingTimeline', totalElapsedSeconds: number, readthroughs: Array<{ __typename?: 'ReadthroughTimeline', readthroughNumber: number, startedAt: any, finishedAt?: any | null, sessions: Array<(
        { __typename?: 'SessionWithEvents', session: { __typename?: 'ReadingSession', id: number, sessionDate: any } }
        & { ' $fragmentRefs'?: { 'ReadingSessionCardFragment': ReadingSessionCardFragment } }
      )> }> } | null, thumbnail: { __typename?: 'ImageRef', url: string, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null } } & { ' $fragmentName'?: 'BookReadingTimelineFragment' };

export type AnnotationEventFragment = { __typename?: 'MediaAnnotation', id: string, annotationText?: string | null, createdAt: any, locator: { __typename?: 'ReadiumLocator', href: string, type: string, chapterTitle: string, locations?: { __typename?: 'ReadiumLocation', position?: number | null, progression?: any | null, totalProgression?: any | null } | null, text?: { __typename?: 'ReadiumText', highlight?: string | null, after?: string | null, before?: string | null } | null } } & { ' $fragmentName'?: 'AnnotationEventFragment' };

export type BookmarkEventFragment = { __typename?: 'Bookmark', id: string, page?: number | null, createdAt: any, bookmarkLocator?: { __typename?: 'ReadiumLocator', locations?: { __typename?: 'ReadiumLocation', position?: number | null } | null } | null } & { ' $fragmentName'?: 'BookmarkEventFragment' };

export type SessionEventsTimelineFragment = { __typename?: 'ReadingSession', id: number, mediaId: string, createdAt: any, startPage?: number | null, updatedAt?: any | null, endPage?: number | null, startLocator?: { __typename?: 'ReadiumLocator', locations?: { __typename?: 'ReadiumLocation', position?: number | null } | null } | null, events: Array<(
    { __typename: 'Bookmark', id: string }
    & { ' $fragmentRefs'?: { 'BookmarkEventFragment': BookmarkEventFragment } }
  ) | (
    { __typename: 'MediaAnnotation', id: string }
    & { ' $fragmentRefs'?: { 'AnnotationEventFragment': AnnotationEventFragment } }
  )>, endLocator?: { __typename?: 'ReadiumLocator', locations?: { __typename?: 'ReadiumLocation', position?: number | null } | null } | null } & { ' $fragmentName'?: 'SessionEventsTimelineFragment' };

export type SessionWithEventsTimelineFragment = { __typename?: 'SessionWithEvents', session: { __typename?: 'ReadingSession', id: number, mediaId: string, createdAt: any, startPage?: number | null, updatedAt?: any | null, endPage?: number | null, startLocator?: { __typename?: 'ReadiumLocator', locations?: { __typename?: 'ReadiumLocation', position?: number | null } | null } | null, endLocator?: { __typename?: 'ReadiumLocator', locations?: { __typename?: 'ReadiumLocation', position?: number | null } | null } | null }, events: Array<(
    { __typename: 'Bookmark', id: string }
    & { ' $fragmentRefs'?: { 'BookmarkEventFragment': BookmarkEventFragment } }
  ) | (
    { __typename: 'MediaAnnotation', id: string }
    & { ' $fragmentRefs'?: { 'AnnotationEventFragment': AnnotationEventFragment } }
  )> } & { ' $fragmentName'?: 'SessionWithEventsTimelineFragment' };

export type UpdateAnnotationMobileEventTimelineMutationVariables = Exact<{
  input: UpdateAnnotationInput;
}>;


export type UpdateAnnotationMobileEventTimelineMutation = { __typename?: 'Mutation', updateAnnotation: { __typename?: 'MediaAnnotation', id: string, annotationText?: string | null, updatedAt: any } };

export type DeleteAnnotationMobileEventTimelineMutationVariables = Exact<{
  id: Scalars['String']['input'];
}>;


export type DeleteAnnotationMobileEventTimelineMutation = { __typename?: 'Mutation', deleteAnnotation: { __typename?: 'MediaAnnotation', id: string } };

export type MyReadingTimelineScreenQueryVariables = Exact<{
  pagination?: InputMaybe<CursorPagination>;
  order?: InputMaybe<OrderDirection>;
}>;


export type MyReadingTimelineScreenQuery = { __typename?: 'Query', myReadingTimeline: { __typename?: 'CursorPaginatedGlobalReadingTimelineNodeResponse', nodes: Array<{ __typename?: 'GlobalReadingTimelineNode', mediaId: string, session: (
        { __typename?: 'SessionWithEvents', session: { __typename?: 'ReadingSession', sessionDate: any } }
        & { ' $fragmentRefs'?: { 'ReadingSessionCardFragment': ReadingSessionCardFragment;'ReadingSessionCardMediaFragment': ReadingSessionCardMediaFragment } }
      ) }> } };

export type ReadingSessionDetailHeaderFragment = { __typename?: 'ReadingSession', id: number, startPage?: number | null, endPage?: number | null, startPercentage?: any | null, endPercentage?: any | null, createdAt: any, updatedAt?: any | null, elapsedSeconds?: number | null, chaptersRead: Array<string>, startLocator?: { __typename?: 'ReadiumLocator', locations?: { __typename?: 'ReadiumLocation', position?: number | null } | null } | null, endLocator?: { __typename?: 'ReadiumLocator', locations?: { __typename?: 'ReadiumLocation', position?: number | null } | null } | null, media?: { __typename?: 'Media', thumbnail: { __typename?: 'ImageRef', url: string, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null } } | null, devices: Array<{ __typename?: 'ReadingDeviceModel', name: string }> } & { ' $fragmentName'?: 'ReadingSessionDetailHeaderFragment' };

export type ReadingSessionMenuFragment = { __typename?: 'ReadingSession', id: number, status: ReadingStatus, mediaId: string, createdAt: any, updatedAt?: any | null, elapsedSeconds?: number | null } & { ' $fragmentName'?: 'ReadingSessionMenuFragment' };

export type SessionJournalFragment = { __typename?: 'ReadingSession', id: number, journalEntry?: { __typename?: 'JournalEntryModel', id: number, content: string, updatedAt: any, mediaId: string } | null } & { ' $fragmentName'?: 'SessionJournalFragment' };

export type UpsertReadingSessionJournalEntryMutationVariables = Exact<{
  sessionId: Scalars['Int']['input'];
  content: Scalars['String']['input'];
}>;


export type UpsertReadingSessionJournalEntryMutation = { __typename?: 'Mutation', upsertReadingSessionJournalEntry: { __typename?: 'JournalEntryModel', content: string, updatedAt: any } };

export type DeleteReadingSessionMutationVariables = Exact<{
  sessionId: Scalars['Int']['input'];
}>;


export type DeleteReadingSessionMutation = { __typename?: 'Mutation', deleteReadingSession: boolean };

export type ExtendReadingSessionMutationVariables = Exact<{
  sessionId: Scalars['Int']['input'];
}>;


export type ExtendReadingSessionMutation = { __typename?: 'Mutation', extendReadingSession: { __typename?: 'ReadingSession', updatedAt?: any | null } };

export type RecentlyAddedSeriesGridQueryVariables = Exact<{
  pagination?: InputMaybe<Pagination>;
}>;


export type RecentlyAddedSeriesGridQuery = { __typename?: 'Query', series: { __typename?: 'PaginatedSeriesResponse', nodes: Array<(
      { __typename?: 'Series', id: string }
      & { ' $fragmentRefs'?: { 'SeriesGridItemFragment': SeriesGridItemFragment } }
    )>, pageInfo: { __typename: 'CursorPaginationInfo' } | { __typename: 'OffsetPaginationInfo', totalPages: number, currentPage: number, pageSize: number, pageOffset: number, zeroBased: boolean } } };

export type RecentlyAddedSeriesItemFragment = { __typename?: 'Series', id: string, createdAt: any, resolvedName: string, mediaCount: number, readCount: number, stackedThumbnails: Array<{ __typename?: 'ImageRef', url: string, height?: number | null, width?: number | null, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null }> } & { ' $fragmentName'?: 'RecentlyAddedSeriesItemFragment' };

export type SeriesGridItemFragment = { __typename?: 'Series', id: string, resolvedName: string, isComplete: boolean, thumbnail: { __typename?: 'ImageRef', url: string, height?: number | null, width?: number | null, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null } } & { ' $fragmentName'?: 'SeriesGridItemFragment' };

export type SeriesListItemFragment = { __typename?: 'Series', id: string, resolvedName: string, isComplete: boolean, mediaCount: number, readCount: number, thumbnail: { __typename?: 'ImageRef', url: string, height?: number | null, width?: number | null, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null } } & { ' $fragmentName'?: 'SeriesListItemFragment' };

export type SeriesOverviewSheetQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type SeriesOverviewSheetQuery = { __typename?: 'Query', seriesById?: { __typename?: 'Series', resolvedName: string, metadata?: { __typename?: 'SeriesMetadata', ageRating?: number | null, booktype?: string | null, characters: Array<string>, comicImage?: string | null, comicid?: number | null, descriptionFormatted?: string | null, genres: Array<string>, imprint?: string | null, links: Array<string>, metaType?: string | null, publicationRun?: string | null, publisher?: string | null, status?: string | null, summary?: string | null, title?: string | null, totalIssues?: number | null, volume?: number | null, writers: Array<string>, year?: number | null, collects: Array<{ __typename?: 'CollectedItem', series?: string | null, comicid?: string | null, issueid?: string | null, issues?: string | null }> } | null, stats: { __typename?: 'SeriesStats', bookCount: number, totalBytes: number, completedBooks: number, inProgressBooks: number, totalReadingTimeSeconds: number }, tags: Array<{ __typename?: 'Tag', name: string }> } | null };

export type SeriesSearchItemFragment = { __typename?: 'Series', id: string, resolvedName: string, readCount: number, mediaCount: number, percentageCompleted: number, thumbnail: { __typename?: 'ImageRef', url: string, height?: number | null, width?: number | null, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null } } & { ' $fragmentName'?: 'SeriesSearchItemFragment' };

export type SmartListBookItemFragment = { __typename?: 'Media', id: string, resolvedName: string, name: string, pages: number, size: number, readProgress?: { __typename?: 'ResumeReadingCursor', page?: number | null, percentageCompleted?: any | null, locator?: { __typename?: 'ReadiumLocator', chapterTitle: string } | null } | null, thumbnail: { __typename?: 'ImageRef', url: string, height?: number | null, width?: number | null, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null } } & { ' $fragmentName'?: 'SmartListBookItemFragment' };

export type SmartListGridItemFragment = { __typename?: 'SmartList', id: string, name: string, description?: string | null, books: Array<{ __typename?: 'Media', thumbnail: { __typename?: 'ImageRef', url: string, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null } }>, meta: { __typename?: 'SmartListMeta', matchedBooks: number, matchedSeries: number, matchedLibraries: number } } & { ' $fragmentName'?: 'SmartListGridItemFragment' };

export type SeriesBooksForDownloadQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type SeriesBooksForDownloadQuery = { __typename?: 'Query', seriesById?: { __typename?: 'Series', id: string, resolvedName: string, library: { __typename?: 'Library', id: string, name: string }, media: Array<{ __typename?: 'Media', id: string, extension: string, resolvedName: string, metadata?: { __typename?: 'MediaMetadata', ageRating?: number | null, characters: Array<string>, colorists: Array<string>, coverArtists: Array<string>, day?: number | null, editors: Array<string>, format?: string | null, identifierAmazon?: string | null, identifierCalibre?: string | null, identifierGoogle?: string | null, identifierIsbn?: string | null, identifierMobiAsin?: string | null, identifierUuid?: string | null, genres: Array<string>, inkers: Array<string>, language?: string | null, letterers: Array<string>, links: Array<string>, month?: number | null, notes?: string | null, number?: any | null, pageCount?: number | null, pencillers: Array<string>, publisher?: string | null, series?: string | null, seriesGroup?: string | null, storyArc?: string | null, storyArcNumber?: any | null, summary?: string | null, teams: Array<string>, title?: string | null, titleSort?: string | null, volume?: number | null, writers: Array<string>, year?: number | null } | null, readProgress?: { __typename?: 'ResumeReadingCursor', page?: number | null, percentageCompleted?: any | null, elapsedSeconds: number, updatedAt?: any | null, locator?: { __typename?: 'ReadiumLocator', chapterTitle: string, href: string, type: string, title?: string | null, locations?: { __typename?: 'ReadiumLocation', fragments?: Array<string> | null, position?: number | null, progression?: any | null, totalProgression?: any | null, cssSelector?: string | null, partialCfi?: string | null } | null } | null } | null, thumbnail: { __typename?: 'ImageRef', metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null }, ebook?: { __typename?: 'Epub', toc: Array<string> } | null }> } | null };

export type UseFavoriteBookMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  isFavorite: Scalars['Boolean']['input'];
}>;


export type UseFavoriteBookMutation = { __typename?: 'Mutation', favoriteMedia: { __typename?: 'Media', id: string, isFavorite: boolean } };

export type UseSyncDeviceNameMutationVariables = Exact<{
  deviceId: Scalars['String']['input'];
  deviceName: Scalars['String']['input'];
}>;


export type UseSyncDeviceNameMutation = { __typename?: 'Mutation', upsertReadingDevice: { __typename?: 'ReadingDeviceModel', id: string, name: string } };

export type TagSelectQueryQueryVariables = Exact<{ [key: string]: never; }>;


export type TagSelectQueryQuery = { __typename?: 'Query', tags: Array<{ __typename?: 'Tag', id: number, name: string }> };

export type BookCardFragment = { __typename?: 'Media', id: string, resolvedName: string, extension: string, pages: number, size: number, status: FileStatus, createdAt: any, thumbnail: { __typename?: 'ImageRef', url: string, height?: number | null, width?: number | null, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null }, readProgress?: { __typename?: 'ResumeReadingCursor', percentageCompleted?: any | null, page?: number | null, updatedAt?: any | null, locator?: { __typename?: 'ReadiumLocator', href: string } | null } | null, readHistory: Array<{ __typename: 'ReadthroughRecord', completedAt: any }>, libraryConfig: { __typename?: 'LibraryConfig', skipBookOverview: boolean }, metadata?: { __typename?: 'MediaMetadata', number?: any | null, rating?: any | null, ageRating?: number | null, language?: string | null } | null } & { ' $fragmentName'?: 'BookCardFragment' };

export type BookHeaderFragment = (
  { __typename?: 'Media', id: string, resolvedName: string, pages: number, extension: string, series?: { __typename?: 'Series', id: string, resolvedName: string } | null, library: { __typename?: 'Library', id: string, name: string } }
  & { ' $fragmentRefs'?: { 'BookReadingActionsFragment': BookReadingActionsFragment } }
) & { ' $fragmentName'?: 'BookHeaderFragment' };

export type BookHeaderAnalyzeMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type BookHeaderAnalyzeMutation = { __typename?: 'Mutation', analyzeMedia: boolean };

export type BookInfoSheetFragment = { __typename?: 'Media', id: string, resolvedName: string, pages: number, size: number, extension: string, hash?: string | null, koreaderHash?: string | null, relativeLibraryPath: string, createdAt: any, modifiedAt?: any | null, library: { __typename?: 'Library', id: string, name: string }, series?: { __typename?: 'Series', id: string, resolvedName: string } | null, metadata?: { __typename?: 'MediaMetadata', publisher?: string | null, year?: number | null, month?: number | null, day?: number | null, language?: string | null, ageRating?: number | null, volume?: number | null, number?: any | null, storyArc?: string | null, storyArcNumber?: any | null, writers: Array<string>, pencillers: Array<string>, inkers: Array<string>, colorists: Array<string>, letterers: Array<string>, coverArtists: Array<string>, editors: Array<string>, characters: Array<string>, teams: Array<string>, identifierIsbn?: string | null, identifierAmazon?: string | null, identifierCalibre?: string | null, identifierGoogle?: string | null, identifierMobiAsin?: string | null, identifierUuid?: string | null } | null, readHistory: Array<{ __typename?: 'ReadthroughRecord', completedAt: any, dnf: boolean }> } & { ' $fragmentName'?: 'BookInfoSheetFragment' };

export type BookOverviewDetailsFragment = (
  { __typename?: 'Media', id: string, resolvedName: string, seriesPosition?: number | null, series?: { __typename?: 'Series', id: string, resolvedName: string } | null, library: { __typename?: 'Library', id: string, name: string, config: { __typename?: 'LibraryConfig', libraryType: LibraryType } }, metadata?: { __typename?: 'MediaMetadata', summary?: string | null, links: Array<string>, genres: Array<string>, writers: Array<string>, pencillers: Array<string>, number?: any | null, volume?: number | null } | null, tags: Array<{ __typename?: 'Tag', id: number, name: string }>, readProgress?: { __typename?: 'ResumeReadingCursor', percentageCompleted?: any | null } | null, readHistory: Array<{ __typename: 'ReadthroughRecord' }> }
  & { ' $fragmentRefs'?: { 'BookStatRowFragment': BookStatRowFragment } }
) & { ' $fragmentName'?: 'BookOverviewDetailsFragment' };

export type BookSearchSelectionFragment = { __typename?: 'Media', id: string, resolvedName: string, thumbnail: { __typename?: 'ImageRef', url: string } } & { ' $fragmentName'?: 'BookSearchSelectionFragment' };

export type BookSearchOverlayQueryVariables = Exact<{
  pagination?: InputMaybe<Pagination>;
  filter: MediaFilterInput;
}>;


export type BookSearchOverlayQuery = { __typename?: 'Query', media: { __typename?: 'PaginatedMediaResponse', nodes: Array<(
      { __typename?: 'Media', id: string }
      & { ' $fragmentRefs'?: { 'BookCardFragment': BookCardFragment;'BookSearchSelectionFragment': BookSearchSelectionFragment } }
    )>, pageInfo: { __typename: 'CursorPaginationInfo', currentCursor?: string | null, nextCursor?: string | null, limit: number } | { __typename: 'OffsetPaginationInfo' } } };

export type BookStatRowFragment = { __typename?: 'Media', pages: number, size: number, extension: string, metadata?: { __typename?: 'MediaMetadata', year?: number | null, rating?: any | null, ageRating?: number | null, language?: string | null } | null } & { ' $fragmentName'?: 'BookStatRowFragment' };

export type EmailBookDropdownDeviceQueryVariables = Exact<{ [key: string]: never; }>;


export type EmailBookDropdownDeviceQuery = { __typename?: 'Query', emailDevices: Array<{ __typename?: 'RegisteredEmailDevice', id: number, name: string }> };

export type SendEmailAttachmentMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  sendTo: Array<EmailerSendTo> | EmailerSendTo;
}>;


export type SendEmailAttachmentMutation = { __typename?: 'Mutation', sendAttachmentEmail: { __typename?: 'SendAttachmentEmailOutput', sentCount: number, errors: Array<string> } };

export type RemoveBookFromLibraryMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type RemoveBookFromLibraryMutation = { __typename?: 'Mutation', deleteMedia: { __typename?: 'Media', id: string } };

export type RemoveBookRetentionQueryVariables = Exact<{ [key: string]: never; }>;


export type RemoveBookRetentionQuery = { __typename?: 'Query', deletedBooksSettings: { __typename?: 'DeletedBooksSettings', retentionDays: number } };

export type SimpleBookCardFragment = { __typename?: 'Media', id: string, resolvedName: string, createdAt: any, thumbnail: { __typename?: 'ImageRef', url: string, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null } } & { ' $fragmentName'?: 'SimpleBookCardFragment' };

export type MediaMetadataEditorFragment = { __typename?: 'MediaMetadata', ageRating?: number | null, characters: Array<string>, colorists: Array<string>, coverArtists: Array<string>, day?: number | null, editors: Array<string>, format?: string | null, identifierAmazon?: string | null, identifierCalibre?: string | null, identifierGoogle?: string | null, identifierIsbn?: string | null, identifierMobiAsin?: string | null, identifierUuid?: string | null, genres: Array<string>, inkers: Array<string>, language?: string | null, letterers: Array<string>, links: Array<string>, month?: number | null, notes?: string | null, number?: any | null, pageCount?: number | null, pencillers: Array<string>, publisher?: string | null, series?: string | null, seriesGroup?: string | null, storyArc?: string | null, storyArcNumber?: any | null, summary?: string | null, teams: Array<string>, title?: string | null, titleSort?: string | null, volume?: number | null, writers: Array<string>, year?: number | null, lockedFields: Array<MetadataField> } & { ' $fragmentName'?: 'MediaMetadataEditorFragment' };

export type MediaEditorSetLockedFieldsMutationVariables = Exact<{
  mediaId: Scalars['ID']['input'];
  lockedFields: Array<MetadataField> | MetadataField;
}>;


export type MediaEditorSetLockedFieldsMutation = { __typename?: 'Mutation', setMediaLockedFields: { __typename?: 'Media', id: string } };

export type MetadataVocabularyQueryVariables = Exact<{
  field: MetadataFacetField;
  libraryId?: InputMaybe<Scalars['ID']['input']>;
  search?: InputMaybe<Scalars['String']['input']>;
}>;


export type MetadataVocabularyQuery = { __typename?: 'Query', metadataValueCounts: Array<{ __typename?: 'MetadataValueCount', value: string, count: number }> };

export type MediaGenreSuggestionsQueryVariables = Exact<{
  id: Scalars['ID']['input'];
  online?: InputMaybe<Scalars['Boolean']['input']>;
}>;


export type MediaGenreSuggestionsQuery = { __typename?: 'Query', mediaGenreSuggestions: { __typename?: 'MediaGenreSuggestions', subjects: Array<string>, lookedUpOnline: boolean, suggestions: Array<{ __typename?: 'IntakeGenreSuggestion', value: string, confidence: number, reasons: Array<string>, libraryCount: number, selected: boolean, kind: string }>, current: Array<{ __typename?: 'CurrentGenreFit', value: string, kind: string, confidence: number, fits: boolean, reasons: Array<string>, libraryCount: number }>, similarBooks: Array<{ __typename?: 'IntakeSimilarBook', mediaId: string, title: string, authors: Array<string>, genres: Array<string>, reason: string }>, onlineLookups: Array<{ __typename?: 'OnlineLookup', source: IntakeSource, status: OnlineLookupStatus, subjects: number, message: string }> } };

export type MediaDescriptionSuggestionsQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type MediaDescriptionSuggestionsQuery = { __typename?: 'Query', mediaDescriptionSuggestions: { __typename?: 'MediaDescriptions', status: MediaDescriptionStatus, searchedTitle?: string | null, searchedAuthors: Array<string>, candidates: Array<{ __typename?: 'MediaDescriptionCandidate', source: IntakeSource, text: string, words: number }>, onlineLookups: Array<{ __typename?: 'OnlineLookup', source: IntakeSource, status: OnlineLookupStatus, subjects: number, message: string }> } };

export type EditMediaMetadataMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  input: MediaMetadataInput;
}>;


export type EditMediaMetadataMutation = { __typename?: 'Mutation', editMediaMetadata: { __typename?: 'MetadataEditResult', saveId?: string | null, media: { __typename?: 'Media', id: string }, file: { __typename?: 'MetadataEditFile', status: MetadataFileWriteStatus, fileFields: Array<string>, stumpOnlyFields: Array<string>, message?: string | null } } };

export type RevertMediaMetadataEditMutationVariables = Exact<{
  mediaId: Scalars['ID']['input'];
  saveId: Scalars['String']['input'];
}>;


export type RevertMediaMetadataEditMutation = { __typename?: 'Mutation', revertMediaMetadataEdit: { __typename?: 'MetadataEditResult', saveId?: string | null, skippedFields: Array<string>, media: { __typename?: 'Media', id: string }, file: { __typename?: 'MetadataEditFile', status: MetadataFileWriteStatus, fileFields: Array<string>, stumpOnlyFields: Array<string>, message?: string | null } } };

export type MediaMetadataEditsQueryVariables = Exact<{
  mediaId: Scalars['ID']['input'];
  limit?: InputMaybe<Scalars['Int']['input']>;
}>;


export type MediaMetadataEditsQuery = { __typename?: 'Query', mediaMetadataEdits: Array<{ __typename?: 'MetadataEditSave', saveId: string, editedAt: any, editedBy?: string | null, revertsSaveId?: string | null, revertedBy?: string | null, changes: Array<{ __typename?: 'MetadataEditChange', field: string, oldValue?: string | null, newValue?: string | null, writtenToFile: boolean }> }> };

export type EditBookDetailsQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type EditBookDetailsQuery = { __typename?: 'Query', mediaById?: { __typename?: 'Media', id: string, resolvedName: string, extension: string, library: { __typename?: 'Library', id: string }, metadata?: { __typename?: 'MediaMetadata', title?: string | null, writers: Array<string>, series?: string | null, number?: any | null, genres: Array<string>, summary?: string | null, publisher?: string | null, year?: number | null, month?: number | null, day?: number | null, language?: string | null, ageRating?: number | null } | null } | null };

export type BookOverviewSceneQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type BookOverviewSceneQuery = { __typename?: 'Query', mediaById?: (
    { __typename?: 'Media', id: string, resolvedName: string, seriesId?: string | null, metadata?: { __typename?: 'MediaMetadata', writers: Array<string> } | null, library: { __typename?: 'Library', config: { __typename?: 'LibraryConfig', libraryType: LibraryType } }, thumbnail: { __typename?: 'ImageRef', url: string, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null } }
    & { ' $fragmentRefs'?: { 'BookHeaderFragment': BookHeaderFragment;'BookOverviewDetailsFragment': BookOverviewDetailsFragment;'BookInfoSheetFragment': BookInfoSheetFragment;'BookReadingActionsFragment': BookReadingActionsFragment } }
  ) | null };

export type BookReadingActionsFragment = { __typename?: 'Media', id: string, extension: string, readProgress?: { __typename?: 'ResumeReadingCursor', page?: number | null, locator?: { __typename?: 'ReadiumLocator', href: string } | null } | null, readHistory: Array<{ __typename: 'ReadthroughRecord' }> } & { ' $fragmentName'?: 'BookReadingActionsFragment' };

export type BookReadingActionsCompleteMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type BookReadingActionsCompleteMutation = { __typename?: 'Mutation', finishMediaProgress: boolean };

export type BookReadingActionsDeleteSessionMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type BookReadingActionsDeleteSessionMutation = { __typename?: 'Mutation', clearMediaProgress: boolean };

export type BookReadingActionsDeleteHistoryMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type BookReadingActionsDeleteHistoryMutation = { __typename?: 'Mutation', deleteMediaReadingHistory: number };

export type BookThumbnailSelectorUpdateMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  input: PageBasedThumbnailInput;
}>;


export type BookThumbnailSelectorUpdateMutation = { __typename?: 'Mutation', updateMediaThumbnail: { __typename?: 'Media', id: string, thumbnail: { __typename?: 'ImageRef', url: string } } };

export type BookThumbnailSelectorUploadMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  file: Scalars['Upload']['input'];
}>;


export type BookThumbnailSelectorUploadMutation = { __typename?: 'Mutation', uploadMediaThumbnail: { __typename?: 'Media', id: string, thumbnail: { __typename?: 'ImageRef', url: string } } };

export type DeleteBookClubConfirmationMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type DeleteBookClubConfirmationMutation = { __typename?: 'Mutation', deleteBookClub: { __typename?: 'BookClub', id: string } };

export type BookClubBookItemFragment = { __typename?: 'BookClubBook', id: string, title?: string | null, author?: string | null, imageUrl?: string | null, url?: string | null, completedAt?: any | null, addedAt: any, entity?: { __typename: 'Media', id: string, resolvedName: string, metadata?: { __typename?: 'MediaMetadata', writers: Array<string> } | null, thumbnail: { __typename?: 'ImageRef', url: string } } | null } & { ' $fragmentName'?: 'BookClubBookItemFragment' };

export type BookClubBooksSceneQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type BookClubBooksSceneQuery = { __typename?: 'Query', bookClubById: { __typename?: 'BookClub', id: string, previousBooks: Array<(
      { __typename?: 'BookClubBook', id: string }
      & { ' $fragmentRefs'?: { 'BookClubBookItemFragment': BookClubBookItemFragment } }
    )> } };

export type MediaAtPathQueryVariables = Exact<{
  path: Scalars['String']['input'];
}>;


export type MediaAtPathQuery = { __typename?: 'Query', mediaByPath?: { __typename?: 'Media', id: string, resolvedName: string, thumbnail: { __typename?: 'ImageRef', url: string } } | null };

export type UploadLibraryBooksMutationVariables = Exact<{
  input: UploadBooksInput;
}>;


export type UploadLibraryBooksMutation = { __typename?: 'Mutation', uploadBooks: boolean };

export type UploadLibrarySeriesMutationVariables = Exact<{
  input: UploadSeriesInput;
}>;


export type UploadLibrarySeriesMutation = { __typename?: 'Mutation', uploadSeries: boolean };

export type MediaFilterFormQueryVariables = Exact<{
  seriesId?: InputMaybe<Scalars['ID']['input']>;
  smartListId?: InputMaybe<Scalars['ID']['input']>;
}>;


export type MediaFilterFormQuery = { __typename?: 'Query', mediaMetadataOverview: { __typename?: 'MediaMetadataOverview', genres: Array<string>, languages: Array<string>, writers: Array<string>, pencillers: Array<string>, colorists: Array<string>, letterers: Array<string>, inkers: Array<string>, publishers: Array<string>, editors: Array<string>, characters: Array<string> }, tags: Array<{ __typename?: 'Tag', id: number, name: string }> };

export type DeleteLibraryMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type DeleteLibraryMutation = { __typename?: 'Mutation', deleteLibrary: { __typename?: 'Library', id: string } };

export type LastVisitedLibraryQueryVariables = Exact<{ [key: string]: never; }>;


export type LastVisitedLibraryQuery = { __typename?: 'Query', lastVisitedLibrary?: { __typename?: 'Library', id: string, name: string, thumbnail: { __typename?: 'ImageRef', url: string } } | null };

export type LibraryBooksAlphabetQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type LibraryBooksAlphabetQuery = { __typename?: 'Query', libraryById?: { __typename?: 'Library', mediaAlphabet: any } | null };

export type LibraryWebGridItemFragment = { __typename?: 'Library', id: string, name: string, series: Array<{ __typename?: 'Series', thumbnail: { __typename?: 'ImageRef', url: string, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null } }>, stats: { __typename?: 'LibraryStats', seriesCount: number, bookCount: number } } & { ' $fragmentName'?: 'LibraryWebGridItemFragment' };

export type LibrarySeriesAlphabetQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type LibrarySeriesAlphabetQuery = { __typename?: 'Query', libraryById?: { __typename?: 'Library', seriesAlphabet: any } | null };

export type PendingMetadataMatchesQueryVariables = Exact<{ [key: string]: never; }>;


export type PendingMetadataMatchesQuery = { __typename?: 'Query', pendingMetadataMatches: Array<(
    { __typename?: 'MetadataFetchRecord' }
    & { ' $fragmentRefs'?: { 'PendingMatchRecordFragment': PendingMatchRecordFragment } }
  )> };

export type AcceptAllPendingMatchesMutationVariables = Exact<{
  strategy?: InputMaybe<MergeStrategy>;
  excludeFields?: InputMaybe<Array<MetadataField> | MetadataField>;
}>;


export type AcceptAllPendingMatchesMutation = { __typename?: 'Mutation', acceptAllPendingMatches: number };

export type RejectAllPendingMatchesMutationVariables = Exact<{ [key: string]: never; }>;


export type RejectAllPendingMatchesMutation = { __typename?: 'Mutation', rejectAllPendingMatches: number };

export type PendingMatchRecordFragment = { __typename?: 'MetadataFetchRecord', id: number, status: MetadataFetchStatus, mediaId?: string | null, seriesId?: string | null, rawHits: number, addedAt: any, updatedAt?: any | null, matchCandidates: Array<{ __typename?: 'MatchCandidate', provider: string, externalId: string, confidence: number, metadata: { __typename: 'ExternalMediaMetadata', title?: string | null, seriesName?: string | null, seriesExternalId?: string | null, summary?: string | null, pageCount?: number | null, number?: number | null, day?: number | null, month?: number | null, year?: number | null, genres?: Array<string> | null, tags?: Array<string> | null, isbn?: string | null, isbn13?: string | null, writers?: Array<string> | null, artists?: Array<string> | null, colorists?: Array<string> | null, letterers?: Array<string> | null, coverArtists?: Array<string> | null } | { __typename: 'ExternalSeriesMetadata', alternativeTitles: Array<string>, summary?: string | null, volumeCount?: number | null, coverUrl?: string | null, status?: PublicationStatus | null, year?: number | null, endYear?: number | null, genres?: Array<string> | null, tags?: Array<string> | null, authors?: Array<string> | null, ageRating?: string | null, publisher?: string | null, seriesTitle: string }, confidenceFactors: Array<{ __typename?: 'ConfidenceFactor', factor: string, weight: number, matched: boolean }> }>, media?: { __typename?: 'Media', id: string, resolvedName: string, metadata?: { __typename?: 'MediaMetadata', title?: string | null, summary?: string | null, series?: string | null, number?: any | null, genres: Array<string>, writers: Array<string>, colorists: Array<string>, letterers: Array<string>, coverArtists: Array<string>, publisher?: string | null, year?: number | null, month?: number | null, day?: number | null, pageCount?: number | null, identifierIsbn?: string | null, lockedFields: Array<MetadataField> } | null } | null, series?: { __typename?: 'Series', id: string, resolvedName: string, metadata?: { __typename?: 'SeriesMetadata', title?: string | null, summary?: string | null, genres: Array<string>, writers: Array<string>, publisher?: string | null, year?: number | null, status?: string | null, ageRating?: number | null, volume?: number | null, lockedFields: Array<MetadataField> } | null } | null } & { ' $fragmentName'?: 'PendingMatchRecordFragment' };

export type AcceptMediaMatchMutationVariables = Exact<{
  mediaId: Scalars['ID']['input'];
  candidateIndex: Scalars['Int']['input'];
  strategy?: InputMaybe<MergeStrategy>;
  excludeFields?: InputMaybe<Array<MetadataField> | MetadataField>;
  overrides?: InputMaybe<Array<MetadataFieldOverride> | MetadataFieldOverride>;
}>;


export type AcceptMediaMatchMutation = { __typename?: 'Mutation', acceptMediaMatch: (
    { __typename?: 'MetadataFetchRecord' }
    & { ' $fragmentRefs'?: { 'PendingMatchRecordFragment': PendingMatchRecordFragment } }
  ) };

export type AcceptSeriesMatchMutationVariables = Exact<{
  seriesId: Scalars['ID']['input'];
  candidateIndex: Scalars['Int']['input'];
  strategy?: InputMaybe<MergeStrategy>;
  excludeFields?: InputMaybe<Array<MetadataField> | MetadataField>;
  overrides?: InputMaybe<Array<MetadataFieldOverride> | MetadataFieldOverride>;
}>;


export type AcceptSeriesMatchMutation = { __typename?: 'Mutation', acceptSeriesMatch: (
    { __typename?: 'MetadataFetchRecord' }
    & { ' $fragmentRefs'?: { 'PendingMatchRecordFragment': PendingMatchRecordFragment } }
  ) };

export type RejectMediaMatchMutationVariables = Exact<{
  mediaId: Scalars['ID']['input'];
  candidateIndex: Scalars['Int']['input'];
}>;


export type RejectMediaMatchMutation = { __typename?: 'Mutation', rejectMediaMatch: (
    { __typename?: 'MetadataFetchRecord' }
    & { ' $fragmentRefs'?: { 'PendingMatchRecordFragment': PendingMatchRecordFragment } }
  ) };

export type RejectSeriesMatchMutationVariables = Exact<{
  seriesId: Scalars['ID']['input'];
  candidateIndex: Scalars['Int']['input'];
}>;


export type RejectSeriesMatchMutation = { __typename?: 'Mutation', rejectSeriesMatch: (
    { __typename?: 'MetadataFetchRecord' }
    & { ' $fragmentRefs'?: { 'PendingMatchRecordFragment': PendingMatchRecordFragment } }
  ) };

export type SetMediaLockedFieldsMutationVariables = Exact<{
  mediaId: Scalars['ID']['input'];
  lockedFields: Array<MetadataField> | MetadataField;
}>;


export type SetMediaLockedFieldsMutation = { __typename?: 'Mutation', setMediaLockedFields: { __typename?: 'Media', id: string } };

export type SetSeriesLockedFieldsMutationVariables = Exact<{
  seriesId: Scalars['ID']['input'];
  lockedFields: Array<MetadataField> | MetadataField;
}>;


export type SetSeriesLockedFieldsMutation = { __typename?: 'Mutation', setSeriesLockedFields: { __typename?: 'Series', id: string } };

export type SideBarQueryQueryVariables = Exact<{ [key: string]: never; }>;


export type SideBarQueryQuery = { __typename?: 'Query', me: { __typename?: 'User', id: string, preferences: { __typename?: 'UserPreferences', navigationArrangement: { __typename?: 'Arrangement', locked: boolean, sections: Array<{ __typename?: 'ArrangementSection', visible: boolean, config: { __typename: 'CustomArrangementConfig' } | { __typename: 'InProgressBooks' } | { __typename: 'OnDeckBooks' } | { __typename: 'RecentlyAdded' } | { __typename: 'SystemArrangementConfig', variant: SystemArrangement, links: Array<FilterableArrangementEntityLink> } }> } } } };

export type BookClubSideBarSectionQueryVariables = Exact<{ [key: string]: never; }>;


export type BookClubSideBarSectionQuery = { __typename?: 'Query', bookClubs: Array<{ __typename?: 'BookClub', id: string, name: string, slug: string, emoji?: string | null, members: Array<{ __typename?: 'BookClubMember', id: string, userId: string, role: BookClubMemberRole }> }> };

export type LibraryEmojiFragment = { __typename?: 'Library', id: string } & { ' $fragmentName'?: 'LibraryEmojiFragment' };

export type UpdateLibraryEmojiMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  emoji?: InputMaybe<Scalars['String']['input']>;
}>;


export type UpdateLibraryEmojiMutation = { __typename?: 'Mutation', updateLibraryEmoji: { __typename?: 'Library', id: string } };

export type LibraryOptionsMenuFragment = { __typename?: 'Library', id: string, name: string } & { ' $fragmentName'?: 'LibraryOptionsMenuFragment' };

export type ScanLibraryMutationMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type ScanLibraryMutationMutation = { __typename?: 'Mutation', scanLibrary: boolean };

export type LibrarySideBarSectionQueryVariables = Exact<{ [key: string]: never; }>;


export type LibrarySideBarSectionQuery = { __typename?: 'Query', libraries: { __typename?: 'PaginatedLibraryResponse', nodes: Array<(
      { __typename?: 'Library', id: string, name: string, emoji?: string | null }
      & { ' $fragmentRefs'?: { 'LibraryEmojiFragment': LibraryEmojiFragment;'LibraryOptionsMenuFragment': LibraryOptionsMenuFragment } }
    )> } };

export type SmartListSideBarSectionQueryVariables = Exact<{ [key: string]: never; }>;


export type SmartListSideBarSectionQuery = { __typename?: 'Query', smartLists: Array<{ __typename?: 'SmartList', id: string, name: string }> };

export type NavigationEntityLibraryQueryVariables = Exact<{
  id: Scalars['ID']['input'];
  isSeries: Scalars['Boolean']['input'];
  isBook: Scalars['Boolean']['input'];
}>;


export type NavigationEntityLibraryQuery = { __typename?: 'Query', seriesById?: { __typename?: 'Series', libraryId?: string | null } | null, mediaById?: { __typename?: 'Media', libraryId: string } | null };

export type PostProcessingTasksQueryVariables = Exact<{
  libraryId?: InputMaybe<Scalars['ID']['input']>;
  limit?: InputMaybe<Scalars['Int']['input']>;
}>;


export type PostProcessingTasksQuery = { __typename?: 'Query', postProcessingTasks: Array<{ __typename?: 'PostProcessingTaskModel', id: string, libraryId: string, mediaId?: string | null, kind: PostProcessKind, steps?: string | null, status: PostProcessStatus, path: string, reason?: string | null, bytesBefore?: number | null, bytesAfter?: number | null, updatedAt: any }> };

export type CreateReaderAnnotationMutationVariables = Exact<{
  input: CreateAnnotationInput;
}>;


export type CreateReaderAnnotationMutation = { __typename?: 'Mutation', createAnnotation: { __typename?: 'MediaAnnotation', id: string, mediaId: string, userId: string, annotationText?: string | null, style: AnnotationStyle, createdAt: any, updatedAt: any, locator: { __typename?: 'ReadiumLocator', chapterTitle: string, href: string, title?: string | null, type: string, locations?: { __typename?: 'ReadiumLocation', fragments?: Array<string> | null, progression?: any | null, position?: number | null, totalProgression?: any | null, cssSelector?: string | null, rects?: Array<{ __typename?: 'ReadiumRect', x: any, y: any, width: any, height: any }> | null } | null, text?: { __typename?: 'ReadiumText', after?: string | null, before?: string | null, highlight?: string | null } | null } } };

export type UpdateReaderAnnotationMutationVariables = Exact<{
  input: UpdateAnnotationInput;
}>;


export type UpdateReaderAnnotationMutation = { __typename?: 'Mutation', updateAnnotation: { __typename?: 'MediaAnnotation', id: string, annotationText?: string | null, style: AnnotationStyle, updatedAt: any } };

export type DeleteReaderAnnotationMutationVariables = Exact<{
  id: Scalars['String']['input'];
}>;


export type DeleteReaderAnnotationMutation = { __typename?: 'Mutation', deleteAnnotation: { __typename?: 'MediaAnnotation', id: string } };

export type CreateBookmarkMutationVariables = Exact<{
  input: BookmarkInput;
}>;


export type CreateBookmarkMutation = { __typename?: 'Mutation', createBookmark: { __typename: 'Bookmark' } };

export type DeleteBookmarkMutationVariables = Exact<{
  id: Scalars['String']['input'];
}>;


export type DeleteBookmarkMutation = { __typename?: 'Mutation', deleteBookmark: { __typename: 'Bookmark' } };

export type ReadiumWebReaderQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type ReadiumWebReaderQuery = { __typename?: 'Query', epubById: { __typename?: 'Epub', mediaId: string, toc: Array<string>, bookmarks: Array<{ __typename?: 'Bookmark', id: string, userId: string, mediaId: string, previewContent?: string | null, createdAt: any, locator?: { __typename?: 'ReadiumLocator', chapterTitle: string, href: string, title?: string | null, type: string, locations?: { __typename?: 'ReadiumLocation', fragments?: Array<string> | null, progression?: any | null, position?: number | null, totalProgression?: any | null, cssSelector?: string | null } | null, text?: { __typename?: 'ReadiumText', after?: string | null, before?: string | null, highlight?: string | null } | null } | null }>, annotations: Array<{ __typename?: 'MediaAnnotationModel', id: string, mediaId: string, userId: string, annotationText?: string | null, style: AnnotationStyle, createdAt: any, updatedAt: any, locator: { __typename?: 'ReadiumLocator', chapterTitle: string, href: string, title?: string | null, type: string, locations?: { __typename?: 'ReadiumLocation', fragments?: Array<string> | null, progression?: any | null, position?: number | null, totalProgression?: any | null, cssSelector?: string | null } | null, text?: { __typename?: 'ReadiumText', after?: string | null, before?: string | null, highlight?: string | null } | null } }>, media: { __typename?: 'Media', id: string, resolvedName: string, pages: number, extension: string, readProgress?: { __typename?: 'ResumeReadingCursor', percentageCompleted?: any | null, page?: number | null, elapsedSeconds: number, locator?: { __typename?: 'ReadiumLocator', chapterTitle: string, href: string, title?: string | null, type: string, locations?: { __typename?: 'ReadiumLocation', fragments?: Array<string> | null, progression?: any | null, position?: number | null, totalProgression?: any | null, cssSelector?: string | null } | null, text?: { __typename?: 'ReadiumText', after?: string | null, before?: string | null, highlight?: string | null } | null } | null } | null, libraryConfig: { __typename?: 'LibraryConfig', defaultReadingImageScaleFit: ReadingImageScaleFit, defaultReadingMode: ReadingMode, defaultReadingDir: ReadingDirection }, nextInSeries: { __typename?: 'PaginatedMediaResponse', nodes: Array<{ __typename?: 'Media', id: string, name: string, thumbnail: { __typename?: 'ImageRef', url: string } }> } } } };

export type UpdateReadiumEpubProgressMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  input: MediaProgressInput;
}>;


export type UpdateReadiumEpubProgressMutation = { __typename?: 'Mutation', updateMediaProgress: { __typename: 'ReadingSession' } };

export type PdfReaderAnnotationsQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type PdfReaderAnnotationsQuery = { __typename?: 'Query', annotationsByMediaId: Array<{ __typename?: 'MediaAnnotation', id: string, mediaId: string, userId: string, annotationText?: string | null, style: AnnotationStyle, createdAt: any, updatedAt: any, locator: { __typename?: 'ReadiumLocator', chapterTitle: string, href: string, title?: string | null, type: string, locations?: { __typename?: 'ReadiumLocation', fragments?: Array<string> | null, progression?: any | null, position?: number | null, totalProgression?: any | null, cssSelector?: string | null, rects?: Array<{ __typename?: 'ReadiumRect', x: any, y: any, width: any, height: any }> | null } | null, text?: { __typename?: 'ReadiumText', after?: string | null, before?: string | null, highlight?: string | null } | null } }> };

export type PdfReaderBookmarksQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type PdfReaderBookmarksQuery = { __typename?: 'Query', bookmarksByMediaId: Array<{ __typename?: 'Bookmark', id: string, page?: number | null, previewContent?: string | null, createdAt: any, locator?: { __typename?: 'ReadiumLocator', chapterTitle: string, href: string, title?: string | null, type: string, locations?: { __typename?: 'ReadiumLocation', fragments?: Array<string> | null, progression?: any | null, position?: number | null, totalProgression?: any | null } | null } | null }> };

export type CreatePdfBookmarkMutationVariables = Exact<{
  input: BookmarkInput;
}>;


export type CreatePdfBookmarkMutation = { __typename?: 'Mutation', createBookmark: { __typename?: 'Bookmark', id: string } };

export type DeletePdfBookmarkMutationVariables = Exact<{
  id: Scalars['String']['input'];
}>;


export type DeletePdfBookmarkMutation = { __typename?: 'Mutation', deleteBookmark: { __typename?: 'Bookmark', id: string } };

export type BookReadingListIdsQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type BookReadingListIdsQuery = { __typename?: 'Query', mediaById?: { __typename?: 'Media', id: string, readingListIds: Array<string> } | null };

export type PickerReadingListsQueryVariables = Exact<{ [key: string]: never; }>;


export type PickerReadingListsQuery = { __typename?: 'Query', readingLists: { __typename?: 'PaginatedReadingListResponse', nodes: Array<{ __typename?: 'ReadingList', id: string, name: string, creatingUserId: string, bookCount: number }> } };

export type CreateReadingListMutationVariables = Exact<{
  input: ReadingListInput;
}>;


export type CreateReadingListMutation = { __typename?: 'Mutation', createReadingList: { __typename?: 'ReadingList', id: string, name: string } };

export type UpdateReadingListMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  input: UpdateReadingListInput;
}>;


export type UpdateReadingListMutation = { __typename?: 'Mutation', updateReadingList: { __typename?: 'ReadingList', id: string } };

export type DeleteReadingListMutationVariables = Exact<{
  id: Scalars['String']['input'];
}>;


export type DeleteReadingListMutation = { __typename?: 'Mutation', deleteReadingList: { __typename?: 'ReadingList', id: string } };

export type AddBooksToReadingListMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  mediaIds: Array<Scalars['ID']['input']> | Scalars['ID']['input'];
}>;


export type AddBooksToReadingListMutation = { __typename?: 'Mutation', addBooksToReadingList: { __typename?: 'ReadingList', id: string } };

export type RemoveBooksFromReadingListMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  mediaIds: Array<Scalars['ID']['input']> | Scalars['ID']['input'];
}>;


export type RemoveBooksFromReadingListMutation = { __typename?: 'Mutation', removeBooksFromReadingList: { __typename?: 'ReadingList', id: string } };

export type SeriesBookGridQueryVariables = Exact<{
  id: Scalars['String']['input'];
  pagination?: InputMaybe<Pagination>;
}>;


export type SeriesBookGridQuery = { __typename?: 'Query', media: { __typename?: 'PaginatedMediaResponse', nodes: Array<{ __typename?: 'Media', id: string, pages: number, extension: string, thumbnail: { __typename?: 'ImageRef', url: string } }>, pageInfo: { __typename: 'CursorPaginationInfo', currentCursor?: string | null, nextCursor?: string | null, limit: number } | { __typename: 'OffsetPaginationInfo' } } };

export type SeriesBooksAlphabetQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type SeriesBooksAlphabetQuery = { __typename?: 'Query', seriesById?: { __typename?: 'Series', mediaAlphabet: any } | null };

export type SeriesActionCompleteMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type SeriesActionCompleteMutation = { __typename?: 'Mutation', finishSeriesProgress: number };

export type SeriesHeaderAnalyzeMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type SeriesHeaderAnalyzeMutation = { __typename?: 'Mutation', analyzeSeries: boolean };

export type SeriesOverviewSheetExtasQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type SeriesOverviewSheetExtasQuery = { __typename?: 'Query', seriesById?: { __typename?: 'Series', id: string, metadata?: { __typename?: 'SeriesMetadata', publisher?: string | null, year?: number | null, summary?: string | null, links: Array<string> } | null, upNext: Array<(
      { __typename?: 'Media', id: string }
      & { ' $fragmentRefs'?: { 'SimpleBookCardFragment': SimpleBookCardFragment } }
    )> } | null };

export type SeriesTagEditorSetTagsMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  tags: Array<Scalars['String']['input']> | Scalars['String']['input'];
}>;


export type SeriesTagEditorSetTagsMutation = { __typename?: 'Mutation', setSeriesTags: { __typename?: 'Series', id: string, tags: Array<{ __typename?: 'Tag', id: number, name: string }> } };

export type SeriesMetadataEditorFragment = { __typename?: 'SeriesMetadata', ageRating?: number | null, booktype?: string | null, characters: Array<string>, comicImage?: string | null, comicid?: number | null, descriptionFormatted?: string | null, genres: Array<string>, imprint?: string | null, links: Array<string>, metaType?: string | null, publicationRun?: string | null, publisher?: string | null, status?: string | null, summary?: string | null, title?: string | null, totalIssues?: number | null, volume?: number | null, writers: Array<string>, year?: number | null, lockedFields: Array<MetadataField>, collects: Array<{ __typename?: 'CollectedItem', series?: string | null, comicid?: string | null, issueid?: string | null, issues?: string | null }> } & { ' $fragmentName'?: 'SeriesMetadataEditorFragment' };

export type UpdateSeriesMetadataMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  input: SeriesMetadataInput;
}>;


export type UpdateSeriesMetadataMutation = { __typename?: 'Mutation', updateSeriesMetadata: { __typename?: 'Series', id: string } };

export type SeriesEditorSetLockedFieldsMutationVariables = Exact<{
  seriesId: Scalars['ID']['input'];
  lockedFields: Array<MetadataField> | MetadataField;
}>;


export type SeriesEditorSetLockedFieldsMutation = { __typename?: 'Mutation', setSeriesLockedFields: { __typename?: 'Series', id: string } };

export type SeriesLayoutQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type SeriesLayoutQuery = { __typename?: 'Query', seriesById?: { __typename?: 'Series', kind: SeriesKind, id: string, path?: string | null, resolvedName: string, resolvedDescription?: string | null, createdAt: any, updatedAt?: any | null, library: { __typename?: 'Library', id: string, name: string }, stats: { __typename?: 'SeriesStats', bookCount: number, completedBooks: number, inProgressBooks: number, totalBytes: number, totalReadingTimeSeconds: number }, tags: Array<{ __typename?: 'Tag', id: number, name: string }>, thumbnail: { __typename?: 'ImageRef', url: string, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null } } | null };

export type SeriesBooksSceneQueryVariables = Exact<{
  filter: MediaFilterInput;
  orderBy: Array<MediaOrderBy> | MediaOrderBy;
  pagination: Pagination;
}>;


export type SeriesBooksSceneQuery = { __typename?: 'Query', media: { __typename?: 'PaginatedMediaResponse', nodes: Array<(
      { __typename?: 'Media', id: string }
      & { ' $fragmentRefs'?: { 'BookCardFragment': BookCardFragment;'BookMetadataFragment': BookMetadataFragment } }
    )>, pageInfo: { __typename: 'CursorPaginationInfo' } | { __typename: 'OffsetPaginationInfo', currentPage: number, totalPages: number, pageSize: number, pageOffset: number, zeroBased: boolean } } };

export type SeriesThumbnailSelectorUpdateMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  input: UpdateThumbnailInput;
}>;


export type SeriesThumbnailSelectorUpdateMutation = { __typename?: 'Mutation', updateSeriesThumbnail: { __typename?: 'Series', id: string, thumbnail: { __typename?: 'ImageRef', url: string } } };

export type SeriesThumbnailSelectorUploadMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  file: Scalars['Upload']['input'];
}>;


export type SeriesThumbnailSelectorUploadMutation = { __typename?: 'Mutation', uploadSeriesThumbnail: { __typename?: 'Series', id: string, thumbnail: { __typename?: 'ImageRef', url: string } } };

export type UseCoreEventSubscriptionVariables = Exact<{ [key: string]: never; }>;


export type UseCoreEventSubscription = { __typename?: 'Subscription', readEvents: { __typename: 'CreatedManySeries', count: number, libraryId: string } | { __typename: 'CreatedMedia' } | { __typename: 'CreatedOrUpdatedManyMedia' } | { __typename: 'DiscoveredMissingLibrary', id: string } | { __typename: 'JobOutput', id: string, output: { __typename: 'AnalyzeMediaOutput' } | { __typename: 'LibraryScanOutput', createdMedia: number, createdSeries: number, updatedMedia: number, updatedSeries: number } | { __typename: 'MetadataFetchJobOutput' } | { __typename: 'PlaceholderGenerationOutput' } | { __typename: 'PostProcessOutput' } | { __typename: 'RescanBooksOutput' } | { __typename: 'SeriesScanOutput', createdMedia: number, updatedMedia: number } | { __typename: 'ThumbnailGenerationOutput' } | { __typename: 'TrashPurgeOutput' } } | { __typename: 'JobStarted', id: string } | { __typename: 'JobUpdate', id: string, status?: JobStatus | null, message?: string | null, completedTasks?: number | null, remainingTasks?: number | null, completedSubtasks?: number | null, totalSubtasks?: number | null, subtitle?: string | null } };

export type UsePreferencesMutationVariables = Exact<{
  input: UpdateUserPreferencesInput;
}>;


export type UsePreferencesMutation = { __typename?: 'Mutation', updateViewerPreferences: { __typename: 'UserPreferences' } };

export type BookMetadataFragment = { __typename?: 'Media', metadata?: { __typename?: 'MediaMetadata', ageRating?: number | null, rating?: any | null, characters: Array<string>, colorists: Array<string>, coverArtists: Array<string>, editors: Array<string>, genres: Array<string>, inkers: Array<string>, letterers: Array<string>, links: Array<string>, pencillers: Array<string>, publisher?: string | null, series?: string | null, teams: Array<string>, writers: Array<string>, year?: number | null, month?: number | null, day?: number | null, volume?: number | null, number?: any | null } | null } & { ' $fragmentName'?: 'BookMetadataFragment' };

export type BooksAlphabetQueryVariables = Exact<{ [key: string]: never; }>;


export type BooksAlphabetQuery = { __typename?: 'Query', mediaAlphabet: any };

export type BooksInSeriesQueryVariables = Exact<{
  seriesId: Scalars['ID']['input'];
  filter: MediaFilterInput;
  orderBy: Array<MediaOrderBy> | MediaOrderBy;
  pagination: Pagination;
}>;


export type BooksInSeriesQuery = { __typename?: 'Query', seriesById?: { __typename?: 'Series', id: string, resolvedName: string } | null, media: { __typename?: 'PaginatedMediaResponse', nodes: Array<(
      { __typename?: 'Media', id: string }
      & { ' $fragmentRefs'?: { 'BookCardFragment': BookCardFragment } }
    )>, pageInfo: { __typename: 'CursorPaginationInfo' } | { __typename: 'OffsetPaginationInfo', currentPage: number, totalPages: number, totalItems: number, pageSize: number, pageOffset: number, zeroBased: boolean } } };

export type MoreByAuthorQueryQueryVariables = Exact<{
  filter: MediaFilterInput;
  pagination?: InputMaybe<Pagination>;
}>;


export type MoreByAuthorQueryQuery = { __typename?: 'Query', media: { __typename?: 'PaginatedMediaResponse', nodes: Array<(
      { __typename?: 'Media', id: string, seriesId?: string | null }
      & { ' $fragmentRefs'?: { 'BookCardFragment': BookCardFragment } }
    )> } };

export type BookReaderSceneQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type BookReaderSceneQuery = { __typename?: 'Query', mediaById?: { __typename?: 'Media', id: string, resolvedName: string, pages: number, extension: string, readProgress?: { __typename?: 'ResumeReadingCursor', percentageCompleted?: any | null, page?: number | null, elapsedSeconds: number } | null, libraryConfig: { __typename?: 'LibraryConfig', defaultReadingImageScaleFit: ReadingImageScaleFit, defaultReadingMode: ReadingMode, defaultReadingDir: ReadingDirection }, metadata?: { __typename?: 'MediaMetadata', genres: Array<string> } | null, analysisData?: { __typename?: 'MediaAnalysisData', dimensions: Array<{ __typename?: 'PageDimension', height: number, width: number }> } | null, nextInSeries: { __typename?: 'PaginatedMediaResponse', nodes: Array<{ __typename?: 'Media', id: string, name: string, thumbnail: { __typename?: 'ImageRef', url: string } }> } } | null };

export type UpdateReadProgressMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  input: MediaProgressInput;
}>;


export type UpdateReadProgressMutation = { __typename?: 'Mutation', updateMediaProgress: { __typename: 'ReadingSession' } };

export type BookMetadataSearchProvidersQueryVariables = Exact<{ [key: string]: never; }>;


export type BookMetadataSearchProvidersQuery = { __typename?: 'Query', metadataProviderConfigs: Array<{ __typename?: 'MetadataProviderConfigModel', id: number, providerType: MetadataProvider, enabled: boolean }> };

export type BookMetadataSearchContextQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type BookMetadataSearchContextQuery = { __typename?: 'Query', mediaById?: { __typename?: 'Media', id: string, series?: { __typename?: 'Series', id: string, metadata?: { __typename?: 'SeriesMetadata', comicid?: number | null } | null } | null } | null };

export type SearchMediaMetadataMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  search?: InputMaybe<MediaMetadataSearchInput>;
}>;


export type SearchMediaMetadataMutation = { __typename?: 'Mutation', fetchMediaMetadata: (
    { __typename?: 'MetadataFetchRecord' }
    & { ' $fragmentRefs'?: { 'PendingMatchRecordFragment': PendingMatchRecordFragment } }
  ) };

export type BookSettingsSceneQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type BookSettingsSceneQuery = { __typename?: 'Query', mediaById?: (
    { __typename?: 'Media', id: string, resolvedName: string, tags: Array<{ __typename?: 'Tag', id: number, name: string }>, metadata?: (
      { __typename?: 'MediaMetadata' }
      & { ' $fragmentRefs'?: { 'MediaMetadataEditorFragment': MediaMetadataEditorFragment } }
    ) | null }
    & { ' $fragmentRefs'?: { 'BookHeaderFragment': BookHeaderFragment } }
  ) | null };

export type BookTagEditorSetTagsMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  tags: Array<Scalars['String']['input']> | Scalars['String']['input'];
}>;


export type BookTagEditorSetTagsMutation = { __typename?: 'Mutation', setMediaTags: { __typename?: 'Media', id: string, tags: Array<{ __typename?: 'Tag', id: number, name: string }> } };

export type BookClubLayoutQueryVariables = Exact<{
  slug: Scalars['String']['input'];
}>;


export type BookClubLayoutQuery = { __typename?: 'Query', bookClubBySlug?: { __typename?: 'BookClub', id: string, name: string, slug: string, description?: string | null, isPrivate: boolean, roleSpec: any, membersCount: number, createdAt: any, creator: { __typename?: 'BookClubMember', id: string, displayName?: string | null, avatarUrl?: string | null }, membership?: { __typename?: 'BookClubMember', role: BookClubMemberRole, avatarUrl?: string | null, isCreator: boolean } | null, currentBook?: (
      { __typename?: 'BookClubBook', id: string, title?: string | null, author?: string | null, imageUrl?: string | null, entity?: { __typename?: 'Media', id: string, thumbnail: { __typename?: 'ImageRef', url: string } } | null }
      & { ' $fragmentRefs'?: { 'BookClubBookItemFragment': BookClubBookItemFragment } }
    ) | null } | null };

export type UpdateBookClubMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  input: UpdateBookClubInput;
}>;


export type UpdateBookClubMutation = { __typename?: 'Mutation', updateBookClub: { __typename?: 'BookClub', id: string, name: string, emoji?: string | null, isPrivate: boolean, roleSpec: any, description?: string | null } };

export type UserBookClubsSceneQueryVariables = Exact<{ [key: string]: never; }>;


export type UserBookClubsSceneQuery = { __typename?: 'Query', bookClubs: Array<{ __typename?: 'BookClub', id: string, name: string, slug: string, description?: string | null, membersCount: number, currentBook?: { __typename?: 'BookClubBook', id: string } | null }> };

export type CreateBookClubFormQueryVariables = Exact<{ [key: string]: never; }>;


export type CreateBookClubFormQuery = { __typename?: 'Query', bookClubs: Array<{ __typename?: 'BookClub', name: string, slug: string }> };

export type CreateBookClubSceneMutationVariables = Exact<{
  input: CreateBookClubInput;
}>;


export type CreateBookClubSceneMutation = { __typename?: 'Mutation', createBookClub: { __typename?: 'BookClub', id: string, slug: string } };

export type BookClubBasicSettingsSceneQueryVariables = Exact<{ [key: string]: never; }>;


export type BookClubBasicSettingsSceneQuery = { __typename?: 'Query', bookClubs: Array<{ __typename?: 'BookClub', id: string, name: string, slug: string }> };

export type BookClubMembersTableQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type BookClubMembersTableQuery = { __typename?: 'Query', bookClubById: { __typename?: 'BookClub', id: string, members: Array<{ __typename?: 'BookClubMember', id: string, avatarUrl?: string | null, isCreator: boolean, displayName?: string | null, role: BookClubMemberRole, userId: string }> } };

export type RemoveBookClubMemberMutationVariables = Exact<{
  bookClubId: Scalars['ID']['input'];
  memberId: Scalars['ID']['input'];
}>;


export type RemoveBookClubMemberMutation = { __typename?: 'Mutation', removeBookClubMember: { __typename?: 'BookClubMember', id: string } };

export type BookSearchSceneQueryVariables = Exact<{
  filter: MediaFilterInput;
  orderBy: Array<MediaOrderBy> | MediaOrderBy;
  pagination: Pagination;
}>;


export type BookSearchSceneQuery = { __typename?: 'Query', media: { __typename?: 'PaginatedMediaResponse', nodes: Array<(
      { __typename?: 'Media', id: string }
      & { ' $fragmentRefs'?: { 'BookCardFragment': BookCardFragment;'BookMetadataFragment': BookMetadataFragment } }
    )>, pageInfo: { __typename: 'CursorPaginationInfo' } | { __typename: 'OffsetPaginationInfo', currentPage: number, totalPages: number, pageSize: number, pageOffset: number, zeroBased: boolean } } };

export type BrowseIndexSceneQueryVariables = Exact<{
  field: MetadataFacetField;
}>;


export type BrowseIndexSceneQuery = { __typename?: 'Query', metadataValueCounts: Array<{ __typename?: 'MetadataValueCount', value: string, count: number }> };

export type CreateLibrarySceneExistingLibrariesQueryVariables = Exact<{ [key: string]: never; }>;


export type CreateLibrarySceneExistingLibrariesQuery = { __typename?: 'Query', libraries: { __typename?: 'PaginatedLibraryResponse', nodes: Array<{ __typename?: 'Library', id: string, name: string, path: string }> } };

export type CreateLibrarySceneCreateLibraryMutationVariables = Exact<{
  input: CreateOrUpdateLibraryInput;
}>;


export type CreateLibrarySceneCreateLibraryMutation = { __typename?: 'Mutation', createLibrary: { __typename?: 'Library', id: string } };

export type CreateSmartListFormQueryVariables = Exact<{ [key: string]: never; }>;


export type CreateSmartListFormQuery = { __typename?: 'Query', smartLists: Array<{ __typename?: 'SmartList', name: string }> };

export type CreateSmartListSceneMutationVariables = Exact<{
  input: SaveSmartListInput;
}>;


export type CreateSmartListSceneMutation = { __typename?: 'Mutation', createSmartList: { __typename?: 'SmartList', id: string, name: string } };

export type ContinueReadingBookFragment = { __typename?: 'Media', id: string, resolvedName: string, pages: number, extension: string, thumbnail: { __typename?: 'ImageRef', url: string, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null }, readProgress?: { __typename?: 'ResumeReadingCursor', percentageCompleted?: any | null, page?: number | null, updatedAt?: any | null, locator?: { __typename?: 'ReadiumLocator', href: string } | null } | null } & { ' $fragmentName'?: 'ContinueReadingBookFragment' };

export type ContinueReadingMediaQueryVariables = Exact<{
  pagination: Pagination;
}>;


export type ContinueReadingMediaQuery = { __typename?: 'Query', keepReading: { __typename?: 'PaginatedMediaResponse', nodes: Array<(
      { __typename?: 'Media', id: string }
      & { ' $fragmentRefs'?: { 'ContinueReadingBookFragment': ContinueReadingBookFragment } }
    )>, pageInfo: { __typename: 'CursorPaginationInfo', currentCursor?: string | null, nextCursor?: string | null, limit: number } | { __typename: 'OffsetPaginationInfo', currentPage: number, totalPages: number, pageSize: number, pageOffset: number, zeroBased: boolean } } };

export type HomeSceneQueryQueryVariables = Exact<{ [key: string]: never; }>;


export type HomeSceneQueryQuery = { __typename?: 'Query', numberOfLibraries: number };

export type HomeSearchQueryVariables = Exact<{
  filter: MediaFilterInput;
}>;


export type HomeSearchQuery = { __typename?: 'Query', media: { __typename?: 'PaginatedMediaResponse', nodes: Array<{ __typename?: 'Media', id: string, resolvedName: string, thumbnail: { __typename?: 'ImageRef', url: string }, metadata?: { __typename?: 'MediaMetadata', writers: Array<string>, rating?: any | null } | null }>, pageInfo: { __typename: 'CursorPaginationInfo' } | { __typename: 'OffsetPaginationInfo', totalPages: number } } };

export type OnDeckBookFragment = { __typename?: 'Media', id: string, resolvedName: string, seriesPosition?: number | null, metadata?: { __typename?: 'MediaMetadata', number?: any | null } | null, series?: { __typename?: 'Series', mediaCount: number, metadata?: { __typename?: 'SeriesMetadata', totalIssues?: number | null } | null } | null, thumbnail: { __typename?: 'ImageRef', url: string, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null } } & { ' $fragmentName'?: 'OnDeckBookFragment' };

export type OnDeckBooksWebQueryVariables = Exact<{
  pagination: Pagination;
}>;


export type OnDeckBooksWebQuery = { __typename?: 'Query', onDeck: { __typename?: 'PaginatedMediaResponse', nodes: Array<(
      { __typename?: 'Media', id: string }
      & { ' $fragmentRefs'?: { 'OnDeckBookFragment': OnDeckBookFragment } }
    )>, pageInfo: { __typename: 'CursorPaginationInfo' } | { __typename: 'OffsetPaginationInfo', currentPage: number, totalPages: number, pageSize: number, pageOffset: number, zeroBased: boolean } } };

export type RecentlyAddedBookFragment = { __typename?: 'Media', id: string, resolvedName: string, createdAt: any, thumbnail: { __typename?: 'ImageRef', url: string, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null } } & { ' $fragmentName'?: 'RecentlyAddedBookFragment' };

export type RecentlyAddedMediaQueryVariables = Exact<{
  pagination: Pagination;
}>;


export type RecentlyAddedMediaQuery = { __typename?: 'Query', recentlyAddedMedia: { __typename?: 'PaginatedMediaResponse', nodes: Array<(
      { __typename?: 'Media', id: string }
      & { ' $fragmentRefs'?: { 'RecentlyAddedBookFragment': RecentlyAddedBookFragment } }
    )>, pageInfo: { __typename: 'CursorPaginationInfo', currentCursor?: string | null, nextCursor?: string | null, limit: number } | { __typename: 'OffsetPaginationInfo' } } };

export type RecentlyAddedSeriesQueryVariables = Exact<{
  pagination: Pagination;
}>;


export type RecentlyAddedSeriesQuery = { __typename?: 'Query', recentlyAddedSeries: { __typename?: 'PaginatedSeriesResponse', nodes: Array<{ __typename?: 'Series', id: string, resolvedName: string, mediaCount: number, percentageCompleted: number, status: FileStatus, createdAt: any, oneshotBook?: { __typename?: 'Media', id: string } | null, stackedThumbnails: Array<{ __typename?: 'ImageRef', url: string, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null }> }>, pageInfo: { __typename: 'CursorPaginationInfo', currentCursor?: string | null, nextCursor?: string | null, limit: number } | { __typename: 'OffsetPaginationInfo' } } };

export type HomeArrangementPreferencesQueryVariables = Exact<{ [key: string]: never; }>;


export type HomeArrangementPreferencesQuery = { __typename?: 'Query', me: { __typename?: 'User', preferences: { __typename?: 'UserPreferences', homeArrangement: { __typename?: 'Arrangement', sections: Array<{ __typename?: 'ArrangementSection', visible: boolean, config: { __typename: 'CustomArrangementConfig' } | { __typename: 'InProgressBooks', name?: string | null } | { __typename: 'OnDeckBooks', name?: string | null } | { __typename: 'RecentlyAdded', entity: FilterableArrangementEntity, name?: string | null } | { __typename: 'SystemArrangementConfig' } }> } } } };

export type UpdateHomeArrangementMutationVariables = Exact<{
  input: HomeArrangementInput;
}>;


export type UpdateHomeArrangementMutation = { __typename?: 'Mutation', updateHomeArrangement: { __typename?: 'HomeArrangement', sections: Array<{ __typename?: 'ArrangementSection', visible: boolean, config: { __typename: 'CustomArrangementConfig' } | { __typename: 'InProgressBooks', name?: string | null } | { __typename: 'OnDeckBooks', name?: string | null } | { __typename: 'RecentlyAdded', entity: FilterableArrangementEntity, name?: string | null } | { __typename: 'SystemArrangementConfig' } }> } };

export type IntakeItemsQueryVariables = Exact<{ [key: string]: never; }>;


export type IntakeItemsQuery = { __typename?: 'Query', intakeItems: Array<{ __typename?: 'IntakeItem', id: string, libraryId: string, libraryName: string, state: IntakeState, kind: IntakeKind, name: string, error?: string | null, createdAt: any, filedAt?: any | null, problems: Array<string>, mediaIds: Array<string>, files: Array<{ __typename?: 'IntakeFile', originalName: string, size: number, format: string, volume?: number | null, title?: string | null }>, proposal?: { __typename?: 'IntakeProposal', seriesKind?: IntakeSeriesKind | null, warnings: Array<string>, confidence: number, lookedUpOnline: boolean, metadata: { __typename?: 'IntakeMetadata', title?: string | null, authors: Array<string>, series?: string | null, seriesIndex?: number | null, language?: string | null, year?: number | null, publisher?: string | null, description?: string | null, isbn?: string | null, subjects: Array<string>, genres: Array<string>, sources: Array<{ __typename?: 'IntakeFieldSource', field: string, source: IntakeSource }> }, authorFolders: Array<{ __typename?: 'IntakeFolderCandidate', folder: string, books: number, confidence: number, reason: string, isNew: boolean }>, seriesFolders: Array<{ __typename?: 'IntakeFolderCandidate', folder: string, books: number, confidence: number, reason: string, isNew: boolean }>, genreSuggestions: Array<{ __typename?: 'IntakeGenreSuggestion', value: string, confidence: number, reasons: Array<string>, libraryCount: number, selected: boolean, kind: string }>, similarBooks: Array<{ __typename?: 'IntakeSimilarBook', mediaId: string, title: string, authors: Array<string>, genres: Array<string>, reason: string }>, duplicates: Array<{ __typename?: 'IntakeDuplicate', mediaId: string, path: string, title: string, reason: string, fileName: string }> } | null, decision?: { __typename?: 'IntakeDecision', seriesKind?: IntakeSeriesKind | null, folder: string, fileNames: Array<string>, volumes: Array<number | null>, metadata: { __typename?: 'IntakeMetadata', title?: string | null, authors: Array<string>, series?: string | null, seriesIndex?: number | null, language?: string | null, year?: number | null, publisher?: string | null, genres: Array<string>, ageRating?: number | null, sources: Array<{ __typename?: 'IntakeFieldSource', field: string, source: IntakeSource }> } } | null, destination?: { __typename?: 'IntakeDestination', relativeDir: string, dirExists: boolean, files: Array<{ __typename?: 'IntakeDestinationFile', originalName: string, relativePath: string, exists: boolean }> } | null, result?: { __typename?: 'IntakeResult', paths: Array<string>, notes: Array<string> } | null }> };

export type IntakeLibrariesQueryVariables = Exact<{ [key: string]: never; }>;


export type IntakeLibrariesQuery = { __typename?: 'Query', intakeLibraries: Array<{ __typename?: 'Library', id: string, name: string }> };

export type IntakeVocabularyQueryVariables = Exact<{
  libraryId: Scalars['ID']['input'];
  search?: InputMaybe<Scalars['String']['input']>;
  limit?: Scalars['Int']['input'];
}>;


export type IntakeVocabularyQuery = { __typename?: 'Query', intakeVocabulary: Array<{ __typename?: 'IntakeVocabularyEntry', value: string, count: number, kind: string }> };

export type IntakeFoldersQueryVariables = Exact<{
  libraryId: Scalars['ID']['input'];
  seriesKind?: InputMaybe<IntakeSeriesKind>;
  search?: InputMaybe<Scalars['String']['input']>;
  limit?: Scalars['Int']['input'];
}>;


export type IntakeFoldersQuery = { __typename?: 'Query', intakeFolders: Array<{ __typename?: 'IntakeFolder', name: string, books: number }> };

export type IntakeUploadMutationVariables = Exact<{
  libraryId: Scalars['ID']['input'];
  files: Array<Scalars['Upload']['input']> | Scalars['Upload']['input'];
  relativePaths?: InputMaybe<Array<Scalars['String']['input']> | Scalars['String']['input']>;
  mode?: IntakeUploadMode;
}>;


export type IntakeUploadMutation = { __typename?: 'Mutation', intakeUpload: Array<{ __typename?: 'IntakeItem', id: string, state: IntakeState }> };

export type IntakeReviewMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  input: IntakeReviewInput;
}>;


export type IntakeReviewMutation = { __typename?: 'Mutation', intakeReview: { __typename?: 'IntakeItem', id: string, state: IntakeState } };

export type IntakeReanalyseMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  online?: InputMaybe<Scalars['Boolean']['input']>;
}>;


export type IntakeReanalyseMutation = { __typename?: 'Mutation', intakeReanalyse: { __typename?: 'IntakeItem', id: string, state: IntakeState } };

export type IntakeAcceptMutationVariables = Exact<{
  ids: Array<Scalars['ID']['input']> | Scalars['ID']['input'];
}>;


export type IntakeAcceptMutation = { __typename?: 'Mutation', intakeAccept: Array<{ __typename?: 'IntakeItem', id: string, state: IntakeState, error?: string | null }> };

export type IntakeDiscardMutationVariables = Exact<{
  ids: Array<Scalars['ID']['input']> | Scalars['ID']['input'];
}>;


export type IntakeDiscardMutation = { __typename?: 'Mutation', intakeDiscard: number };

export type LibrariesSceneQueryVariables = Exact<{ [key: string]: never; }>;


export type LibrariesSceneQuery = { __typename?: 'Query', libraries: { __typename?: 'PaginatedLibraryResponse', nodes: Array<(
      { __typename?: 'Library', id: string }
      & { ' $fragmentRefs'?: { 'LibraryWebGridItemFragment': LibraryWebGridItemFragment } }
    )> }, librariesStats: { __typename?: 'LibraryStats', seriesCount: number, bookCount: number } };

export type LibrariesSceneScanAllLibrariesMutationVariables = Exact<{ [key: string]: never; }>;


export type LibrariesSceneScanAllLibrariesMutation = { __typename?: 'Mutation', scanAllLibraries: boolean };

export type LibraryLayoutQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type LibraryLayoutQuery = { __typename?: 'Query', libraryById?: (
    { __typename?: 'Library', id: string, name: string, description?: string | null, path: string, stats: { __typename?: 'LibraryStats', seriesCount: number, bookCount: number, completedBooks: number, inProgressBooks: number, totalBytes: number, totalReadingTimeSeconds: number }, tags: Array<{ __typename?: 'Tag', id: number, name: string }>, thumbnail: { __typename?: 'ImageRef', url: string, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null }, config: { __typename?: 'LibraryConfig', organizationMode: OrganizationMode, defaultLibraryViewMode: LibraryViewMode, hideSeriesView: boolean, oneshotsDirectory?: string | null } }
    & { ' $fragmentRefs'?: { 'LibrarySettingsConfigFragment': LibrarySettingsConfigFragment } }
  ) | null };

export type VisitLibraryMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type VisitLibraryMutation = { __typename?: 'Mutation', visitLibrary: { __typename?: 'Library', id: string } };

export type LibraryBooksSceneQueryVariables = Exact<{
  filter: MediaFilterInput;
  orderBy: Array<MediaOrderBy> | MediaOrderBy;
  pagination: Pagination;
}>;


export type LibraryBooksSceneQuery = { __typename?: 'Query', media: { __typename?: 'PaginatedMediaResponse', nodes: Array<(
      { __typename?: 'Media', id: string }
      & { ' $fragmentRefs'?: { 'BookCardFragment': BookCardFragment;'BookMetadataFragment': BookMetadataFragment } }
    )>, pageInfo: { __typename: 'CursorPaginationInfo' } | { __typename: 'OffsetPaginationInfo', currentPage: number, totalPages: number, pageSize: number, pageOffset: number, zeroBased: boolean } } };

export type LibrarySeriesQueryVariables = Exact<{
  filter: SeriesFilterInput;
  orderBy: Array<SeriesOrderBy> | SeriesOrderBy;
  pagination: Pagination;
}>;


export type LibrarySeriesQuery = { __typename?: 'Query', series: { __typename?: 'PaginatedSeriesResponse', nodes: Array<{ __typename?: 'Series', id: string, resolvedName: string, mediaCount: number, percentageCompleted: number, status: FileStatus, stackedThumbnails: Array<{ __typename?: 'ImageRef', url: string, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null }>, oneshotBook?: { __typename?: 'Media', id: string } | null }>, pageInfo: { __typename: 'CursorPaginationInfo' } | { __typename: 'OffsetPaginationInfo', totalPages: number, currentPage: number, pageSize: number, pageOffset: number, zeroBased: boolean } } };

export type LibrarySeriesGridQueryVariables = Exact<{
  id: Scalars['String']['input'];
  pagination?: InputMaybe<Pagination>;
}>;


export type LibrarySeriesGridQuery = { __typename?: 'Query', series: { __typename?: 'PaginatedSeriesResponse', nodes: Array<{ __typename?: 'Series', id: string, thumbnail: { __typename?: 'ImageRef', url: string } }>, pageInfo: { __typename: 'CursorPaginationInfo', currentCursor?: string | null, nextCursor?: string | null, limit: number } | { __typename: 'OffsetPaginationInfo' } } };

export type LibrarySettingsConfigFragment = { __typename?: 'Library', config: { __typename?: 'LibraryConfig', id: number, convertRarToZip: boolean, hardDeleteConversions: boolean, defaultReadingDir: ReadingDirection, defaultReadingMode: ReadingMode, defaultReadingImageScaleFit: ReadingImageScaleFit, defaultLibraryViewMode: LibraryViewMode, hideSeriesView: boolean, skipBookOverview: boolean, generateFileHashes: boolean, generateKoreaderHashes: boolean, processMetadata: boolean, watch: boolean, libraryPattern: LibraryPattern, organizationMode: OrganizationMode, libraryType: LibraryType, processThumbnailColorsEvenWithoutConfig: boolean, ignoreRules?: Array<string> | null, oneshotsDirectory?: string | null, thumbnailConfig?: { __typename: 'ImageProcessorOptions', format: SupportedImageFormat, quality?: number | null, page?: number | null, resizeMethod?: { __typename: 'ExactDimensionResize', width: number, height: number } | { __typename: 'FitWithinResize' } | { __typename: 'ScaleEvenlyByFactor', factor: any } | { __typename: 'ScaledDimensionResize', dimension: Dimension, size: number } | null } | null } } & { ' $fragmentName'?: 'LibrarySettingsConfigFragment' };

export type LibrarySettingsRouterEditLibraryMutationMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  input: CreateOrUpdateLibraryInput;
}>;


export type LibrarySettingsRouterEditLibraryMutationMutation = { __typename?: 'Mutation', updateLibrary: { __typename?: 'Library', id: string } };

export type LibrarySettingsRouterScanLibraryMutationMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  options?: InputMaybe<Scalars['JSON']['input']>;
}>;


export type LibrarySettingsRouterScanLibraryMutationMutation = { __typename?: 'Mutation', scanLibrary: boolean };

export type BasicSettingsSceneExistingLibrariesQueryVariables = Exact<{ [key: string]: never; }>;


export type BasicSettingsSceneExistingLibrariesQuery = { __typename?: 'Query', libraries: { __typename?: 'PaginatedLibraryResponse', nodes: Array<{ __typename?: 'Library', id: string, name: string, path: string }> } };

export type LibraryExclusionsUsersQueryQueryVariables = Exact<{ [key: string]: never; }>;


export type LibraryExclusionsUsersQueryQuery = { __typename?: 'Query', users: { __typename?: 'PaginatedUserResponse', nodes: Array<{ __typename?: 'User', id: string, username: string }> } };

export type LibraryExclusionsQueryQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type LibraryExclusionsQueryQuery = { __typename?: 'Query', libraryById?: { __typename?: 'Library', excludedUsers: Array<{ __typename?: 'User', id: string, username: string }> } | null };

export type UpdateLibraryExclusionsMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  userIds: Array<Scalars['String']['input']> | Scalars['String']['input'];
}>;


export type UpdateLibraryExclusionsMutation = { __typename?: 'Mutation', updateLibraryExcludedUsers: { __typename?: 'Library', id: string, excludedUsers: Array<{ __typename?: 'User', id: string, username: string }> } };

export type CleanLibraryMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type CleanLibraryMutation = { __typename?: 'Mutation', cleanLibrary: { __typename?: 'CleanLibraryResponse', deletedMediaCount: number, deletedSeriesCount: number, isEmpty: boolean } };

export type LibraryDeletedBooksQueryVariables = Exact<{
  libraryId: Scalars['ID']['input'];
}>;


export type LibraryDeletedBooksQuery = { __typename?: 'Query', deletedBooks: Array<{ __typename?: 'DeletedBook', book: { __typename?: 'Media', id: string } }>, deletedBooksSettings: { __typename?: 'DeletedBooksSettings', retentionDays: number } };

export type LibraryMissingEntitiesQueryVariables = Exact<{
  libraryId: Scalars['ID']['input'];
  pagination: Pagination;
}>;


export type LibraryMissingEntitiesQuery = { __typename?: 'Query', libraryMissingEntities: { __typename?: 'PaginatedMissingEntityResponse', nodes: Array<{ __typename?: 'MissingEntity', id: string, name: string, path?: string | null, type: MissingEntityType }>, pageInfo: { __typename: 'CursorPaginationInfo' } | { __typename: 'OffsetPaginationInfo', totalPages: number, currentPage: number, pageSize: number, pageOffset: number, zeroBased: boolean, totalItems: number } } };

export type AnalyzeLibraryMediaMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type AnalyzeLibraryMediaMutation = { __typename?: 'Mutation', analyzeLibrary: boolean };

export type InitFetchJobCheckProvidersQueryVariables = Exact<{ [key: string]: never; }>;


export type InitFetchJobCheckProvidersQuery = { __typename?: 'Query', metadataProviderConfigs: Array<{ __typename?: 'MetadataProviderConfigModel', id: number }> };

export type InitFetchJobMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type InitFetchJobMutation = { __typename?: 'Mutation', fetchLibraryMetadata: boolean };

export type LibraryAddBooksSettingsQueryVariables = Exact<{
  libraryId: Scalars['ID']['input'];
}>;


export type LibraryAddBooksSettingsQuery = { __typename?: 'Query', libraryIntakeSettings: { __typename?: 'LibraryIntakeSettings', libraryId: string, mangaDir: string, lightNovelDir: string, onlineLookup: boolean } };

export type UpdateLibraryAddBooksSettingsMutationVariables = Exact<{
  libraryId: Scalars['ID']['input'];
  input: LibraryIntakeSettingsInput;
}>;


export type UpdateLibraryAddBooksSettingsMutation = { __typename?: 'Mutation', updateLibraryIntakeSettings: { __typename?: 'LibraryIntakeSettings', libraryId: string } };

export type LibraryPostProcessingQueryVariables = Exact<{
  libraryId: Scalars['ID']['input'];
}>;


export type LibraryPostProcessingQuery = { __typename?: 'Query', libraryPostProcessing: { __typename?: 'LibraryPostProcessing', bytesSaved: number, config: { __typename?: 'PostProcessingConfig', grey16: PostProcessMode, grey16Exclude: Array<string>, pdfOcr: PostProcessMode, pdfShrink: PostProcessMode, grey16Since?: any | null, pdfOcrSince?: any | null, pdfShrinkSince?: any | null }, pdfWorker: { __typename?: 'PdfWorker', alive: boolean, state?: string | null, version?: string | null, lastSeen?: any | null }, counts: Array<{ __typename?: 'PostProcessingCount', kind: PostProcessKind, status: PostProcessStatus, count: number }> } };

export type UpdateLibraryPostProcessingMutationVariables = Exact<{
  libraryId: Scalars['ID']['input'];
  input: PostProcessingConfigInput;
}>;


export type UpdateLibraryPostProcessingMutation = { __typename?: 'Mutation', updateLibraryPostProcessing: { __typename?: 'LibraryPostProcessing', bytesSaved: number } };

export type RunPostProcessingMutationVariables = Exact<{
  libraryId: Scalars['ID']['input'];
  grey16: Scalars['Boolean']['input'];
  pdfOcr: Scalars['Boolean']['input'];
  pdfShrink: Scalars['Boolean']['input'];
  dryRun: Scalars['Boolean']['input'];
}>;


export type RunPostProcessingMutation = { __typename?: 'Mutation', runPostProcessing: boolean };

export type ScanHistorySectionClearHistoryMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type ScanHistorySectionClearHistoryMutation = { __typename?: 'Mutation', clearScanHistory: number };

export type ScanHistoryTableQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type ScanHistoryTableQuery = { __typename?: 'Query', libraryById?: { __typename?: 'Library', id: string, scanHistory: Array<{ __typename?: 'LibraryScanRecord', id: number, jobId?: string | null, timestamp: any, options?: any | null }> } | null };

export type ScanRecordInspectorJobsQueryVariables = Exact<{
  id: Scalars['ID']['input'];
  loadLogs: Scalars['Boolean']['input'];
}>;


export type ScanRecordInspectorJobsQuery = { __typename?: 'Query', jobById?: { __typename?: 'Job', id: string, outputData?: { __typename: 'AnalyzeMediaOutput' } | { __typename: 'LibraryScanOutput', totalFiles: number, totalDirectories: number, ignoredFiles: number, skippedFiles: number, ignoredDirectories: number, createdMedia: number, updatedMedia: number, createdSeries: number, updatedSeries: number } | { __typename: 'MetadataFetchJobOutput' } | { __typename: 'PlaceholderGenerationOutput' } | { __typename: 'PostProcessOutput' } | { __typename: 'RescanBooksOutput' } | { __typename: 'SeriesScanOutput' } | { __typename: 'ThumbnailGenerationOutput' } | { __typename: 'TrashPurgeOutput' } | null, logs?: Array<{ __typename?: 'Log', id: number }> } | null };

export type DeleteLibraryThumbnailsMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type DeleteLibraryThumbnailsMutation = { __typename?: 'Mutation', deleteLibraryThumbnails: boolean };

export type LibraryThumbnailSelectorUpdateMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  input: UpdateThumbnailInput;
}>;


export type LibraryThumbnailSelectorUpdateMutation = { __typename?: 'Mutation', updateLibraryThumbnail: { __typename?: 'Library', id: string, thumbnail: { __typename?: 'ImageRef', url: string } } };

export type LibraryThumbnailSelectorUploadMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  file: Scalars['Upload']['input'];
}>;


export type LibraryThumbnailSelectorUploadMutation = { __typename?: 'Mutation', uploadLibraryThumbnail: { __typename?: 'Library', id: string, thumbnail: { __typename?: 'ImageRef', url: string } } };

export type ProcessLibraryThumbnailsMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  forceRegenerate: Scalars['Boolean']['input'];
}>;


export type ProcessLibraryThumbnailsMutation = { __typename?: 'Mutation', processLibraryThumbnails: boolean };

export type RegenerateThumbnailsMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  forceRegenerate: Scalars['Boolean']['input'];
}>;


export type RegenerateThumbnailsMutation = { __typename?: 'Mutation', generateLibraryThumbnails: boolean };

export type ReadingListGridItemFragment = { __typename?: 'ReadingList', id: string, name: string, bookCount: number, books: Array<{ __typename?: 'Media', id: string, thumbnail: { __typename?: 'ImageRef', url: string, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null } }> } & { ' $fragmentName'?: 'ReadingListGridItemFragment' };

export type ReadingListSceneQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type ReadingListSceneQuery = { __typename?: 'Query', readingListById: { __typename?: 'ReadingList', id: string, name: string, description?: string | null, creatingUserId: string, bookCount: number, books: Array<(
      { __typename?: 'Media', id: string, resolvedName: string }
      & { ' $fragmentRefs'?: { 'BookCardFragment': BookCardFragment } }
    )> } };

export type ReadingListsSceneQueryVariables = Exact<{ [key: string]: never; }>;


export type ReadingListsSceneQuery = { __typename?: 'Query', readingLists: { __typename?: 'PaginatedReadingListResponse', nodes: Array<(
      { __typename?: 'ReadingList', id: string, name: string, creatingUserId: string }
      & { ' $fragmentRefs'?: { 'ReadingListGridItemFragment': ReadingListGridItemFragment } }
    )> } };

export type SeriesSettingsSceneQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type SeriesSettingsSceneQuery = { __typename?: 'Query', seriesById?: { __typename?: 'Series', id: string, tags: Array<{ __typename?: 'Tag', id: number, name: string }>, metadata?: (
      { __typename?: 'SeriesMetadata' }
      & { ' $fragmentRefs'?: { 'SeriesMetadataEditorFragment': SeriesMetadataEditorFragment } }
    ) | null } | null };

export type SeriesSettingsSceneResetMetadataMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  impact: MetadataResetImpact;
}>;


export type SeriesSettingsSceneResetMetadataMutation = { __typename?: 'Mutation', resetSeriesMetadata: { __typename?: 'Series', id: string } };

export type ApiKeyTableQueryVariables = Exact<{ [key: string]: never; }>;


export type ApiKeyTableQuery = { __typename?: 'Query', apiKeys: Array<{ __typename?: 'Apikey', id: number, name: string, lastUsedAt?: any | null, expiresAt?: any | null, createdAt: any, permissions: { __typename: 'InheritPermissionStruct' } | { __typename: 'UserPermissionStruct', value: Array<UserPermission> } }> };

export type CreateApiKeyModalMutationVariables = Exact<{
  input: ApikeyInput;
}>;


export type CreateApiKeyModalMutation = { __typename?: 'Mutation', createApiKey: { __typename?: 'CreatedAPIKey', secret: string, apiKey: { __typename?: 'Apikey', id: number } } };

export type DeleteApiKeyConfirmModalMutationVariables = Exact<{
  id: Scalars['Int']['input'];
}>;


export type DeleteApiKeyConfirmModalMutation = { __typename?: 'Mutation', deleteApiKey: { __typename?: 'Apikey', id: number } };

export type UploadUserAvatarMutationVariables = Exact<{
  file: Scalars['Upload']['input'];
}>;


export type UploadUserAvatarMutation = { __typename?: 'Mutation', uploadUserAvatar: { __typename?: 'User', id: string, avatar: { __typename?: 'ImageRef', url: string } } };

export type DeleteUserAvatarMutationVariables = Exact<{ [key: string]: never; }>;


export type DeleteUserAvatarMutation = { __typename?: 'Mutation', deleteUserAvatar: { __typename?: 'User', id: string, avatar: { __typename?: 'ImageRef', url: string } } };

export type UpdateUserProfileFormMutationVariables = Exact<{
  input: UpdateUserInput;
}>;


export type UpdateUserProfileFormMutation = { __typename?: 'Mutation', updateViewer: { __typename?: 'User', id: string, username: string } };

export type NavigationArrangementQueryVariables = Exact<{ [key: string]: never; }>;


export type NavigationArrangementQuery = { __typename?: 'Query', me: { __typename?: 'User', preferences: { __typename?: 'UserPreferences', navigationArrangement: { __typename?: 'Arrangement', locked: boolean, sections: Array<{ __typename: 'ArrangementSection', visible: boolean, config: { __typename: 'CustomArrangementConfig' } | { __typename: 'InProgressBooks' } | { __typename: 'OnDeckBooks' } | { __typename: 'RecentlyAdded' } | { __typename: 'SystemArrangementConfig', variant: SystemArrangement, links: Array<FilterableArrangementEntityLink> } }> } } } };

export type NavigationArrangementUpdateMutationVariables = Exact<{
  input: NavigationArrangementInput;
}>;


export type NavigationArrangementUpdateMutation = { __typename?: 'Mutation', updateNavigationArrangement: { __typename: 'Arrangement' } };

export type NavigationArrangementUpdateLockStatusMutationVariables = Exact<{
  locked: Scalars['Boolean']['input'];
}>;


export type NavigationArrangementUpdateLockStatusMutation = { __typename?: 'Mutation', updateNavigationArrangementLock: { __typename: 'Arrangement' } };

export type DeletedBooksQueryVariables = Exact<{
  libraryId?: InputMaybe<Scalars['ID']['input']>;
}>;


export type DeletedBooksQuery = { __typename?: 'Query', deletedBooks: Array<{ __typename?: 'DeletedBook', deletedAt: any, deletedBy?: string | null, expiresAt?: any | null, purgeAt?: any | null, libraryId: string, libraryName: string, relativePath: string, fileExists: boolean, book: { __typename?: 'Media', id: string, resolvedName: string, extension: string, size: number, thumbnail: { __typename?: 'ImageRef', url: string }, metadata?: { __typename?: 'MediaMetadata', writers: Array<string> } | null } }> };

export type DeletedBooksSettingsQueryVariables = Exact<{ [key: string]: never; }>;


export type DeletedBooksSettingsQuery = { __typename?: 'Query', deletedBooksSettings: { __typename?: 'DeletedBooksSettings', retentionDays: number, nextPurgeAt?: any | null, lastPurgeAt?: any | null, lastPurge?: { __typename?: 'DeletedBooksPurgeSummary', finishedAt: any, jobId?: string | null, purged: number, missingFiles: number, failed: number, originals: number, errors: Array<{ __typename?: 'DeletedBooksPurgeError', mediaId: string, title: string, error: string }> } | null } };

export type PurgedBooksQueryVariables = Exact<{
  limit: Scalars['Int']['input'];
}>;


export type PurgedBooksQuery = { __typename?: 'Query', purgedBooks: Array<{ __typename?: 'PurgedBook', id: number, mediaId: string, title: string, authors: Array<string>, libraryName?: string | null, originalPath: string, bytes?: number | null, purgedBy?: string | null, automatic: boolean, purgedAt: any }> };

export type HeldFilesQueryVariables = Exact<{
  libraryId?: InputMaybe<Scalars['ID']['input']>;
}>;


export type HeldFilesQuery = { __typename?: 'Query', heldFiles: Array<{ __typename?: 'HeldFile', id: number, reason: HeldFileReason, steps?: string | null, title: string, libraryId?: string | null, libraryName?: string | null, relativePath: string, bytes: number, heldAt: any, heldBy?: string | null, expiresAt?: any | null, purgeAt?: any | null, canPutBack: boolean, book?: { __typename?: 'Media', id: string, resolvedName: string, thumbnail: { __typename?: 'ImageRef', url: string } } | null }> };

export type PutBackHeldFileMutationVariables = Exact<{
  id: Scalars['Int']['input'];
}>;


export type PutBackHeldFileMutation = { __typename?: 'Mutation', putBackHeldFile: { __typename?: 'HeldFilePutBack', book: { __typename?: 'Media', id: string, resolvedName: string } } };

export type DeleteHeldFilesMutationVariables = Exact<{
  ids: Array<Scalars['Int']['input']> | Scalars['Int']['input'];
}>;


export type DeleteHeldFilesMutation = { __typename?: 'Mutation', deleteHeldFiles: { __typename?: 'HeldFilesDeleteResult', deleted: number, freedBytes: number, kept: Array<{ __typename?: 'HeldFileKept', id: number, title: string, error: string }> } };

export type DeletedBooksLibrariesQueryVariables = Exact<{ [key: string]: never; }>;


export type DeletedBooksLibrariesQuery = { __typename?: 'Query', libraries: { __typename?: 'PaginatedLibraryResponse', nodes: Array<{ __typename?: 'Library', id: string, name: string }> } };

export type RestoreDeletedBooksMutationVariables = Exact<{
  ids: Array<Scalars['ID']['input']> | Scalars['ID']['input'];
}>;


export type RestoreDeletedBooksMutation = { __typename?: 'Mutation', restoreDeletedBooks: number };

export type PurgeDeletedBooksMutationVariables = Exact<{
  ids: Array<Scalars['ID']['input']> | Scalars['ID']['input'];
}>;


export type PurgeDeletedBooksMutation = { __typename?: 'Mutation', purgeDeletedBooks: { __typename?: 'DeletedBooksPurgeResult', purged: number, missingFiles: number, freedBytes: number, kept: Array<{ __typename?: 'DeletedBooksPurgeError', mediaId: string, title: string, error: string }> } };

export type EmptyDeletedBooksMutationVariables = Exact<{
  libraryId?: InputMaybe<Scalars['ID']['input']>;
}>;


export type EmptyDeletedBooksMutation = { __typename?: 'Mutation', emptyDeletedBooks: { __typename?: 'DeletedBooksPurgeResult', purged: number, missingFiles: number, freedBytes: number, kept: Array<{ __typename?: 'DeletedBooksPurgeError', mediaId: string, title: string, error: string }> } };

export type UpdateDeletedBooksRetentionMutationVariables = Exact<{
  days: Scalars['Int']['input'];
}>;


export type UpdateDeletedBooksRetentionMutation = { __typename?: 'Mutation', updateDeletedBooksRetention: { __typename?: 'DeletedBooksSettings', retentionDays: number } };

export type CreateEmailerSceneEmailersQueryVariables = Exact<{ [key: string]: never; }>;


export type CreateEmailerSceneEmailersQuery = { __typename?: 'Query', emailers: Array<{ __typename?: 'Emailer', name: string }> };

export type CreateEmailerSceneCreateEmailerMutationVariables = Exact<{
  input: EmailerInput;
}>;


export type CreateEmailerSceneCreateEmailerMutation = { __typename?: 'Mutation', createEmailer: { __typename?: 'Emailer', id: number } };

export type EditEmailerSceneQueryVariables = Exact<{
  id: Scalars['Int']['input'];
}>;


export type EditEmailerSceneQuery = { __typename?: 'Query', emailers: Array<{ __typename?: 'Emailer', name: string }>, emailerById?: (
    { __typename?: 'Emailer', id: number }
    & { ' $fragmentRefs'?: { 'CreateOrUpdateEmailerFormFragment': CreateOrUpdateEmailerFormFragment } }
  ) | null };

export type EditEmailerSceneEditEmailerMutationVariables = Exact<{
  id: Scalars['Int']['input'];
  input: EmailerInput;
}>;


export type EditEmailerSceneEditEmailerMutation = { __typename?: 'Mutation', updateEmailer: { __typename?: 'Emailer', id: number } };

export type CreateOrUpdateDeviceModalCreateEmailDeviceMutationVariables = Exact<{
  input: EmailDeviceInput;
}>;


export type CreateOrUpdateDeviceModalCreateEmailDeviceMutation = { __typename?: 'Mutation', createEmailDevice: { __typename?: 'RegisteredEmailDevice', id: number, name: string } };

export type CreateOrUpdateDeviceModalUpdateEmailDeviceMutationVariables = Exact<{
  id: Scalars['Int']['input'];
  input: EmailDeviceInput;
}>;


export type CreateOrUpdateDeviceModalUpdateEmailDeviceMutation = { __typename?: 'Mutation', updateEmailDevice: { __typename?: 'RegisteredEmailDevice', id: number, name: string, forbidden: boolean } };

export type DeleteDeviceConfirmationDeleteEmailDeviceMutationVariables = Exact<{
  id: Scalars['Int']['input'];
}>;


export type DeleteDeviceConfirmationDeleteEmailDeviceMutation = { __typename?: 'Mutation', deleteEmailDevice: { __typename?: 'RegisteredEmailDevice', id: number } };

export type EmailDevicesTableQueryVariables = Exact<{ [key: string]: never; }>;


export type EmailDevicesTableQuery = { __typename?: 'Query', emailDevices: Array<{ __typename?: 'RegisteredEmailDevice', id: number, name: string, email: string, forbidden: boolean }> };

export type CreateOrUpdateEmailerFormFragment = { __typename?: 'Emailer', name: string, isPrimary: boolean, smtpHost: string, smtpPort: number, maxAttachmentSizeBytes?: number | null, senderDisplayName: string, senderEmail: string, tlsEnabled: boolean, username: string } & { ' $fragmentName'?: 'CreateOrUpdateEmailerFormFragment' };

export type EmailerListItemFragment = { __typename?: 'Emailer', id: number, name: string, isPrimary: boolean, smtpHost: string, smtpPort: number, lastUsedAt?: any | null, maxAttachmentSizeBytes?: number | null, senderDisplayName: string, senderEmail: string, tlsEnabled: boolean, username: string } & { ' $fragmentName'?: 'EmailerListItemFragment' };

export type DeleteEmailerMutationVariables = Exact<{
  emailerId: Scalars['Int']['input'];
}>;


export type DeleteEmailerMutation = { __typename?: 'Mutation', deleteEmailer: { __typename?: 'Emailer', id: number } };

export type EmailerSendHistoryQueryVariables = Exact<{
  id: Scalars['Int']['input'];
  fetchUser: Scalars['Boolean']['input'];
}>;


export type EmailerSendHistoryQuery = { __typename?: 'Query', emailerById?: { __typename?: 'Emailer', sendHistory: Array<{ __typename?: 'EmailerSendRecord', sentAt: any, recipientEmail: string, sentByUserId?: string | null, sentBy?: { __typename?: 'User', id: string, username: string } | null, attachmentMeta: Array<{ __typename?: 'AttachmentMeta', filename: string, mediaId?: string | null, size: number, media?: { __typename?: 'Media', resolvedName: string } | null }> }> } | null };

export type EmailersListQueryVariables = Exact<{ [key: string]: never; }>;


export type EmailersListQuery = { __typename?: 'Query', emailers: Array<(
    { __typename?: 'Emailer', id: number }
    & { ' $fragmentRefs'?: { 'EmailerListItemFragment': EmailerListItemFragment } }
  )> };

export type TestEmailerMutationVariables = Exact<{
  config: EmailerClientConfig;
  recipient: Scalars['String']['input'];
}>;


export type TestEmailerMutation = { __typename?: 'Mutation', testEmailer: boolean };

export type ServerEmojisSectionQueryVariables = Exact<{ [key: string]: never; }>;


export type ServerEmojisSectionQuery = { __typename?: 'Query', customEmojis: Array<{ __typename?: 'CustomEmoji', id: number, name: string, isAnimated: boolean, url: string }> };

export type ServerEmojisSectionUploadEmojiMutationVariables = Exact<{
  input: CreateCustomEmojiInput;
  upload: Scalars['Upload']['input'];
}>;


export type ServerEmojisSectionUploadEmojiMutation = { __typename?: 'Mutation', uploadCustomEmoji: { __typename?: 'CustomEmoji', id: number, name: string, isAnimated: boolean, url: string } };

export type ServerEmojisSectionRenameEmojiMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  input: UpdateCustomEmojiInput;
}>;


export type ServerEmojisSectionRenameEmojiMutation = { __typename?: 'Mutation', updateCustomEmoji: { __typename?: 'CustomEmoji', id: number, name: string, isAnimated: boolean, url: string } };

export type ServerEmojisSectionDeleteEmojiMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type ServerEmojisSectionDeleteEmojiMutation = { __typename?: 'Mutation', deleteCustomEmoji: boolean };

export type ServerPublicUrlUpdateMutationVariables = Exact<{
  publicUrl: Scalars['String']['input'];
}>;


export type ServerPublicUrlUpdateMutation = { __typename?: 'Mutation', updatePublicUrl: { __typename?: 'ServerConfigModel', publicUrl?: string | null } };

export type ServerPublicUrlQueryVariables = Exact<{ [key: string]: never; }>;


export type ServerPublicUrlQuery = { __typename?: 'Query', serverConfig: { __typename?: 'ServerConfigModel', publicUrl?: string | null } };

export type ServerStatsQueryVariables = Exact<{ [key: string]: never; }>;


export type ServerStatsQuery = { __typename?: 'Query', numberOfLibraries: number, numberOfSeries: number, mediaCount: number, mediaDiskUsage: number };

export type CreateScheduledJobMutationVariables = Exact<{
  input: CreateScheduledJobInput;
}>;


export type CreateScheduledJobMutation = { __typename?: 'Mutation', createScheduledJob: (
    { __typename?: 'ScheduledJob' }
    & { ' $fragmentRefs'?: { 'ScheduledJobRowFragment': ScheduledJobRowFragment } }
  ) };

export type UpdateScheduledJobMutationVariables = Exact<{
  id: Scalars['Int']['input'];
  input: UpdateScheduledJobInput;
}>;


export type UpdateScheduledJobMutation = { __typename?: 'Mutation', updateScheduledJob: (
    { __typename?: 'ScheduledJob' }
    & { ' $fragmentRefs'?: { 'ScheduledJobRowFragment': ScheduledJobRowFragment } }
  ) };

export type DeleteJobHistoryConfirmationMutationVariables = Exact<{ [key: string]: never; }>;


export type DeleteJobHistoryConfirmationMutation = { __typename?: 'Mutation', deleteJobHistory: { __typename?: 'DeleteJobHistory', affectedRows: number } };

export type JobActionMenuCancelJobMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type JobActionMenuCancelJobMutation = { __typename?: 'Mutation', cancelJob: boolean };

export type JobActionMenuDeleteJobMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type JobActionMenuDeleteJobMutation = { __typename?: 'Mutation', cancelJob: boolean };

export type JobActionMenuDeleteLogsMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type JobActionMenuDeleteLogsMutation = { __typename?: 'Mutation', deleteJobLogs: { __typename?: 'DeleteJobAssociatedLogs', affectedRows: number } };

type JobDataInspector_AnalyzeMediaOutput_Fragment = { __typename: 'AnalyzeMediaOutput' } & { ' $fragmentName'?: 'JobDataInspector_AnalyzeMediaOutput_Fragment' };

type JobDataInspector_LibraryScanOutput_Fragment = { __typename: 'LibraryScanOutput', totalFiles: number, totalDirectories: number, ignoredFiles: number, skippedFiles: number, ignoredDirectories: number, createdMedia: number, updatedMedia: number, createdSeries: number, updatedSeries: number } & { ' $fragmentName'?: 'JobDataInspector_LibraryScanOutput_Fragment' };

type JobDataInspector_MetadataFetchJobOutput_Fragment = { __typename: 'MetadataFetchJobOutput' } & { ' $fragmentName'?: 'JobDataInspector_MetadataFetchJobOutput_Fragment' };

type JobDataInspector_PlaceholderGenerationOutput_Fragment = { __typename: 'PlaceholderGenerationOutput' } & { ' $fragmentName'?: 'JobDataInspector_PlaceholderGenerationOutput_Fragment' };

type JobDataInspector_PostProcessOutput_Fragment = { __typename: 'PostProcessOutput' } & { ' $fragmentName'?: 'JobDataInspector_PostProcessOutput_Fragment' };

type JobDataInspector_RescanBooksOutput_Fragment = { __typename: 'RescanBooksOutput', rescannedBooks: number, updatedBooks: number, missingBooks: number } & { ' $fragmentName'?: 'JobDataInspector_RescanBooksOutput_Fragment' };

type JobDataInspector_SeriesScanOutput_Fragment = { __typename: 'SeriesScanOutput', totalFiles: number, ignoredFiles: number, skippedFiles: number, createdMedia: number, updatedMedia: number } & { ' $fragmentName'?: 'JobDataInspector_SeriesScanOutput_Fragment' };

type JobDataInspector_ThumbnailGenerationOutput_Fragment = { __typename: 'ThumbnailGenerationOutput', visitedFiles: number, skippedFiles: number, generatedThumbnails: number, removedThumbnails: number } & { ' $fragmentName'?: 'JobDataInspector_ThumbnailGenerationOutput_Fragment' };

type JobDataInspector_TrashPurgeOutput_Fragment = { __typename: 'TrashPurgeOutput', purgedBooks: number, missingBookFiles: number, keptBooks: number, purgedHeldFiles: number, freedBytes: number } & { ' $fragmentName'?: 'JobDataInspector_TrashPurgeOutput_Fragment' };

export type JobDataInspectorFragment = JobDataInspector_AnalyzeMediaOutput_Fragment | JobDataInspector_LibraryScanOutput_Fragment | JobDataInspector_MetadataFetchJobOutput_Fragment | JobDataInspector_PlaceholderGenerationOutput_Fragment | JobDataInspector_PostProcessOutput_Fragment | JobDataInspector_RescanBooksOutput_Fragment | JobDataInspector_SeriesScanOutput_Fragment | JobDataInspector_ThumbnailGenerationOutput_Fragment | JobDataInspector_TrashPurgeOutput_Fragment;

export type JobDataInspectorLogsQueryVariables = Exact<{
  id: Scalars['String']['input'];
}>;


export type JobDataInspectorLogsQuery = { __typename?: 'Query', logs: { __typename?: 'PaginatedLogResponse', nodes: Array<{ __typename?: 'Log', id: number, level: LogLevel, message: string, timestamp: any }> } };

export type ScheduledJobsQueryVariables = Exact<{ [key: string]: never; }>;


export type ScheduledJobsQuery = { __typename?: 'Query', libraries: { __typename?: 'PaginatedLibraryResponse', nodes: Array<{ __typename?: 'Library', id: string, name: string, emoji?: string | null }> }, scheduledJobs: Array<(
    { __typename?: 'ScheduledJob', id: number, name: string }
    & { ' $fragmentRefs'?: { 'ScheduledJobRowFragment': ScheduledJobRowFragment } }
  )> };

export type DeleteScheduledJobMutationVariables = Exact<{
  id: Scalars['Int']['input'];
}>;


export type DeleteScheduledJobMutation = { __typename?: 'Mutation', deleteScheduledJob: boolean };

export type JobTableQueryVariables = Exact<{
  pagination: Pagination;
}>;


export type JobTableQuery = { __typename?: 'Query', jobs: { __typename?: 'PaginatedJobResponse', nodes: Array<{ __typename?: 'Job', id: string, name: string, description?: string | null, status: JobStatus, createdAt: any, completedAt?: any | null, msElapsed: number, logCount: number, outputData?: (
        { __typename?: 'AnalyzeMediaOutput' }
        & { ' $fragmentRefs'?: { 'JobDataInspector_AnalyzeMediaOutput_Fragment': JobDataInspector_AnalyzeMediaOutput_Fragment } }
      ) | (
        { __typename?: 'LibraryScanOutput' }
        & { ' $fragmentRefs'?: { 'JobDataInspector_LibraryScanOutput_Fragment': JobDataInspector_LibraryScanOutput_Fragment } }
      ) | (
        { __typename?: 'MetadataFetchJobOutput' }
        & { ' $fragmentRefs'?: { 'JobDataInspector_MetadataFetchJobOutput_Fragment': JobDataInspector_MetadataFetchJobOutput_Fragment } }
      ) | (
        { __typename?: 'PlaceholderGenerationOutput' }
        & { ' $fragmentRefs'?: { 'JobDataInspector_PlaceholderGenerationOutput_Fragment': JobDataInspector_PlaceholderGenerationOutput_Fragment } }
      ) | (
        { __typename?: 'PostProcessOutput' }
        & { ' $fragmentRefs'?: { 'JobDataInspector_PostProcessOutput_Fragment': JobDataInspector_PostProcessOutput_Fragment } }
      ) | (
        { __typename?: 'RescanBooksOutput' }
        & { ' $fragmentRefs'?: { 'JobDataInspector_RescanBooksOutput_Fragment': JobDataInspector_RescanBooksOutput_Fragment } }
      ) | (
        { __typename?: 'SeriesScanOutput' }
        & { ' $fragmentRefs'?: { 'JobDataInspector_SeriesScanOutput_Fragment': JobDataInspector_SeriesScanOutput_Fragment } }
      ) | (
        { __typename?: 'ThumbnailGenerationOutput' }
        & { ' $fragmentRefs'?: { 'JobDataInspector_ThumbnailGenerationOutput_Fragment': JobDataInspector_ThumbnailGenerationOutput_Fragment } }
      ) | (
        { __typename?: 'TrashPurgeOutput' }
        & { ' $fragmentRefs'?: { 'JobDataInspector_TrashPurgeOutput_Fragment': JobDataInspector_TrashPurgeOutput_Fragment } }
      ) | null }>, pageInfo: { __typename: 'CursorPaginationInfo' } | { __typename: 'OffsetPaginationInfo', currentPage: number, totalPages: number, pageSize: number, pageOffset: number, zeroBased: boolean } } };

export type ScheduledJobRowFragment = { __typename?: 'ScheduledJob', id: number, name: string, kind: ScheduledJobKind, schedule: string, config?: any | null, enabled: boolean, createdAt: any, lastRunAt?: any | null } & { ' $fragmentName'?: 'ScheduledJobRowFragment' };

export type LiveLogsFeedSubscriptionVariables = Exact<{ [key: string]: never; }>;


export type LiveLogsFeedSubscription = { __typename?: 'Subscription', tailLogFile: string };

export type DeleteLogsMutationVariables = Exact<{ [key: string]: never; }>;


export type DeleteLogsMutation = { __typename?: 'Mutation', deleteLogs: { __typename?: 'LogDeleteOutput', deleted: number } };

export type PersistedLogsQueryVariables = Exact<{
  filter: LogFilterInput;
  pagination: Pagination;
  orderBy: Array<LogModelOrderBy> | LogModelOrderBy;
}>;


export type PersistedLogsQuery = { __typename?: 'Query', logs: { __typename?: 'PaginatedLogResponse', nodes: Array<{ __typename?: 'Log', id: number, timestamp: any, level: LogLevel, message: string, jobId?: string | null, context?: string | null }>, pageInfo: { __typename: 'CursorPaginationInfo' } | { __typename: 'OffsetPaginationInfo', totalPages: number, currentPage: number, pageSize: number, pageOffset: number, zeroBased: boolean } } };

export type OnlineLookupSettingsQueryVariables = Exact<{ [key: string]: never; }>;


export type OnlineLookupSettingsQuery = { __typename?: 'Query', onlineLookupSettings: { __typename?: 'OnlineLookupSettings', googleBooksApiKeySet: boolean } };

export type UpdateOnlineLookupSettingsMutationVariables = Exact<{
  googleBooksApiKey?: InputMaybe<Scalars['String']['input']>;
}>;


export type UpdateOnlineLookupSettingsMutation = { __typename?: 'Mutation', updateOnlineLookupSettings: { __typename?: 'OnlineLookupSettings', googleBooksApiKeySet: boolean } };

export type CreateProviderDialogCreateProviderMutationVariables = Exact<{
  input: CreateMetadataProviderConfigInput;
}>;


export type CreateProviderDialogCreateProviderMutation = { __typename?: 'Mutation', createMetadataProvider: { __typename?: 'MetadataProviderConfigModel', id: number, providerType: MetadataProvider, enabled: boolean } };

export type EditProviderDialogFragment = { __typename?: 'MetadataProviderConfigModel', id: number, providerType: MetadataProvider, enabled: boolean, apiTokenExpiresAt?: any | null, autoApplyConfig?: any | null } & { ' $fragmentName'?: 'EditProviderDialogFragment' };

export type EditProviderDialogMutationVariables = Exact<{
  id: Scalars['Int']['input'];
  input: PatchMetadataProviderConfigInput;
}>;


export type EditProviderDialogMutation = { __typename?: 'Mutation', updateMetadataProvider: (
    { __typename?: 'MetadataProviderConfigModel', id: number }
    & { ' $fragmentRefs'?: { 'ExistingProviderCardFragment': ExistingProviderCardFragment } }
  ) };

export type DeleteProviderDialogMutationVariables = Exact<{
  id: Scalars['Int']['input'];
}>;


export type DeleteProviderDialogMutation = { __typename?: 'Mutation', deleteMetadataProvider: { __typename?: 'MetadataProviderConfigModel', id: number } };

export type ExistingProviderCardFragment = (
  { __typename?: 'MetadataProviderConfigModel', id: number, providerType: MetadataProvider, enabled: boolean, apiTokenExpiresAt?: any | null, autoApplyConfig?: any | null, createdAt: any, updatedAt?: any | null }
  & { ' $fragmentRefs'?: { 'EditProviderDialogFragment': EditProviderDialogFragment } }
) & { ' $fragmentName'?: 'ExistingProviderCardFragment' };

export type MetadataFetchSettingsQueryVariables = Exact<{ [key: string]: never; }>;


export type MetadataFetchSettingsQuery = { __typename?: 'Query', metadataFetchSettings: { __typename?: 'MetadataFetchSettings', fetchAfterScan: boolean } };

export type UpdateMetadataFetchSettingsMutationVariables = Exact<{
  fetchAfterScan: Scalars['Boolean']['input'];
}>;


export type UpdateMetadataFetchSettingsMutation = { __typename?: 'Mutation', updateMetadataFetchSettings: { __typename?: 'MetadataFetchSettings', fetchAfterScan: boolean } };

export type ProviderApiKeyInputValidateKeyMutationVariables = Exact<{
  config: ValidateMetadataProviderConfigInput;
}>;


export type ProviderApiKeyInputValidateKeyMutation = { __typename?: 'Mutation', validateProviderConfig: { __typename?: 'ProviderCredentialVerification', isValid: boolean, error?: string | null, responseStatus: number } };

export type ProvidersSectionGetProvidersQueryVariables = Exact<{ [key: string]: never; }>;


export type ProvidersSectionGetProvidersQuery = { __typename?: 'Query', metadataProviderConfigs: Array<(
    { __typename?: 'MetadataProviderConfigModel', id: number }
    & { ' $fragmentRefs'?: { 'ExistingProviderCardFragment': ExistingProviderCardFragment } }
  )> };

export type AdminRecentlyAddedBooksQueryVariables = Exact<{
  userId?: InputMaybe<Scalars['ID']['input']>;
  limit?: Scalars['Int']['input'];
}>;


export type AdminRecentlyAddedBooksQuery = { __typename?: 'Query', recentlyAddedBooks: Array<{ __typename?: 'RecentlyAddedBook', itemId: string, userId: string, username?: string | null, title: string, authors: Array<string>, series?: string | null, paths: Array<string>, libraryId: string, libraryName: string, filedAt?: any | null, mediaIds: Array<string> }> };

export type CreateTagModalMutationVariables = Exact<{
  tags: Array<Scalars['String']['input']> | Scalars['String']['input'];
}>;


export type CreateTagModalMutation = { __typename?: 'Mutation', createTags: Array<{ __typename?: 'Tag', id: number, name: string }> };

export type DeleteTagConfirmModalMutationVariables = Exact<{
  tags: Array<Scalars['String']['input']> | Scalars['String']['input'];
}>;


export type DeleteTagConfirmModalMutation = { __typename?: 'Mutation', deleteTags: Array<{ __typename?: 'Tag', id: number, name: string }> };

export type RenameTagModalMutationVariables = Exact<{
  id: Scalars['Int']['input'];
  name: Scalars['String']['input'];
}>;


export type RenameTagModalMutation = { __typename?: 'Mutation', renameTag: { __typename?: 'Tag', id: number, name: string } };

export type TagTableQueryVariables = Exact<{ [key: string]: never; }>;


export type TagTableQuery = { __typename?: 'Query', tags: Array<{ __typename?: 'Tag', id: number, name: string }> };

export type UserStatsQueryVariables = Exact<{ [key: string]: never; }>;


export type UserStatsQuery = { __typename?: 'Query', userCount: number, activeReadingSessionCount: number, finishedReadingSessionCount: number, topReaders: Array<{ __typename?: 'User', id: string, username: string, finishedReadingSessionsCount: number }> };

export type CreateOrUpdateUserFormUpdateUserMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  input: UpdateUserInput;
}>;


export type CreateOrUpdateUserFormUpdateUserMutation = { __typename?: 'Mutation', updateUser: { __typename?: 'User', id: string, username: string, permissions: Array<UserPermission>, maxSessionsAllowed?: number | null, ageRestriction?: { __typename?: 'AgeRestriction', age: number, restrictOnUnset: boolean } | null } };

export type CreateOrUpdateUserFormCreateUserMutationVariables = Exact<{
  input: CreateUserInput;
}>;


export type CreateOrUpdateUserFormCreateUserMutation = { __typename?: 'Mutation', createUser: { __typename?: 'User', id: string } };

export type CreateUserSceneQueryVariables = Exact<{ [key: string]: never; }>;


export type CreateUserSceneQuery = { __typename?: 'Query', users: { __typename?: 'PaginatedUserResponse', nodes: Array<{ __typename?: 'User', username: string }> } };

export type UpdateUserSceneQueryVariables = Exact<{
  id: Scalars['ID']['input'];
  skip: Scalars['Boolean']['input'];
}>;


export type UpdateUserSceneQuery = { __typename?: 'Query', me: { __typename?: 'User', id: string }, userById?: { __typename?: 'User', id: string, avatarUrl?: string | null, username: string, permissions: Array<UserPermission>, maxSessionsAllowed?: number | null, isServerOwner: boolean, ageRestriction?: { __typename?: 'AgeRestriction', age: number, restrictOnUnset: boolean } | null }, users?: { __typename?: 'PaginatedUserResponse', nodes: Array<{ __typename?: 'User', username: string }> } };

export type ClearLoginActivityConfirmationMutationVariables = Exact<{ [key: string]: never; }>;


export type ClearLoginActivityConfirmationMutation = { __typename?: 'Mutation', deleteLoginActivity: number };

export type LoginActivityTableQueryVariables = Exact<{ [key: string]: never; }>;


export type LoginActivityTableQuery = { __typename?: 'Query', loginActivity: Array<{ __typename?: 'UserLoginActivity', id: number, ipAddress: string, userAgent: string, authenticationSuccessful: boolean, timestamp: any, user: { __typename?: 'User', id: string, username: string, avatarUrl?: string | null } }> };

export type DeleteUserMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  hardDelete?: InputMaybe<Scalars['Boolean']['input']>;
}>;


export type DeleteUserMutation = { __typename?: 'Mutation', deleteUser: { __typename?: 'User', id: string } };

export type UserActionMenuLockUserMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  lock: Scalars['Boolean']['input'];
}>;


export type UserActionMenuLockUserMutation = { __typename?: 'Mutation', updateUserLockStatus: { __typename?: 'User', id: string, isLocked: boolean } };

export type UserActionMenuDeleteUserSessionsMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type UserActionMenuDeleteUserSessionsMutation = { __typename?: 'Mutation', deleteUserSessions: number };

export type UserTableQueryVariables = Exact<{
  pagination: Pagination;
}>;


export type UserTableQuery = { __typename?: 'Query', users: { __typename?: 'PaginatedUserResponse', nodes: Array<{ __typename?: 'User', id: string, avatarUrl?: string | null, username: string, isServerOwner: boolean, isLocked: boolean, createdAt: any, lastLogin?: any | null, loginSessionsCount: number }>, pageInfo: { __typename: 'CursorPaginationInfo' } | { __typename: 'OffsetPaginationInfo', totalPages: number, currentPage: number, pageSize: number, pageOffset: number, zeroBased: boolean } } };

export type SmartListWebGridItemFragment = { __typename?: 'SmartList', id: string, name: string, books: Array<{ __typename?: 'Media', thumbnail: { __typename?: 'ImageRef', url: string, metadata?: { __typename?: 'ImageMetadata', averageColor?: string | null, thumbhash?: string | null, colors: Array<{ __typename?: 'ImageColor', color: string, percentage: any }> } | null } }>, meta: { __typename?: 'SmartListMeta', matchedBooks: number, matchedSeries: number, matchedLibraries: number } } & { ' $fragmentName'?: 'SmartListWebGridItemFragment' };

export type SmartListsWithSearchQueryVariables = Exact<{
  input: SmartListsInput;
}>;


export type SmartListsWithSearchQuery = { __typename?: 'Query', smartLists: Array<(
    { __typename?: 'SmartList', id: string }
    & { ' $fragmentRefs'?: { 'SmartListWebGridItemFragment': SmartListWebGridItemFragment } }
  )> };

export type SmartListByIdQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type SmartListByIdQuery = { __typename?: 'Query', smartListById?: { __typename?: 'SmartList', id: string, creatorId: string, description?: string | null, defaultGrouping: SmartListGrouping, filters: string, joiner: SmartListJoiner, name: string, visibility: EntityVisibility, views: Array<{ __typename?: 'SmartListView', id: number, listId: string, name: string, search?: string | null, bookColumns: Array<{ __typename?: 'SmartListViewColumn', id: string, position: number }>, bookSorting: Array<{ __typename?: 'SmartListViewSort', id: string, desc: boolean }>, groupColumns: Array<{ __typename?: 'SmartListViewColumn', id: string, position: number }>, groupSorting: Array<{ __typename?: 'SmartListViewSort', id: string, desc: boolean }> }> } | null };

export type SmartListMetaQueryVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type SmartListMetaQuery = { __typename?: 'Query', smartListMeta?: { __typename?: 'SmartListMeta', matchedBooks: number, matchedSeries: number, matchedLibraries: number } | null };

export type UpdateSmartListMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  input: SaveSmartListInput;
}>;


export type UpdateSmartListMutation = { __typename?: 'Mutation', updateSmartList: { __typename: 'SmartList' } };

export type SmartListItemsQueryVariables = Exact<{
  id: Scalars['ID']['input'];
  filter?: InputMaybe<MediaFilterInput>;
  grouping?: InputMaybe<SmartListGrouping>;
}>;


export type SmartListItemsQuery = { __typename?: 'Query', smartListItems: { __typename: 'SmartListGrouped', items: Array<{ __typename?: 'SmartListGroupedItem', entity?: { __typename: 'Library', id: string, name: string } | { __typename: 'Series', id: string, name: string } | null, books: Array<(
        { __typename?: 'Media', path: string, tags: Array<{ __typename?: 'Tag', id: number, name: string }> }
        & { ' $fragmentRefs'?: { 'BookCardFragment': BookCardFragment;'BookMetadataFragment': BookMetadataFragment } }
      )> }> } | { __typename: 'SmartListUngrouped', books: Array<(
      { __typename?: 'Media', path: string, tags: Array<{ __typename?: 'Tag', id: number, name: string }> }
      & { ' $fragmentRefs'?: { 'BookCardFragment': BookCardFragment;'SmartListItemBookMetadataFragment': SmartListItemBookMetadataFragment } }
    )> } };

export type SmartListItemBookMetadataFragment = { __typename?: 'Media', metadata?: { __typename?: 'MediaMetadata', ageRating?: number | null, rating?: any | null, characters: Array<string>, colorists: Array<string>, coverArtists: Array<string>, editors: Array<string>, genres: Array<string>, inkers: Array<string>, letterers: Array<string>, links: Array<string>, pencillers: Array<string>, publisher?: string | null, teams: Array<string>, writers: Array<string>, year?: number | null, month?: number | null, day?: number | null, format?: string | null, identifierAmazon?: string | null, identifierCalibre?: string | null, identifierGoogle?: string | null, identifierIsbn?: string | null, identifierMobiAsin?: string | null, identifierUuid?: string | null, language?: string | null, notes?: string | null, number?: any | null, pageCount?: number | null, series?: string | null, seriesGroup?: string | null, storyArc?: string | null, storyArcNumber?: any | null, title?: string | null, titleSort?: string | null, volume?: number | null } | null } & { ' $fragmentName'?: 'SmartListItemBookMetadataFragment' };

export type CreateSmartListViewMutationVariables = Exact<{
  input: SaveSmartListView;
}>;


export type CreateSmartListViewMutation = { __typename?: 'Mutation', createSmartListView: { __typename?: 'SmartListView', id: number, listId: string, name: string, search?: string | null, enableMultiSort?: boolean | null, bookColumns: Array<{ __typename?: 'SmartListViewColumn', id: string, position: number }>, bookSorting: Array<{ __typename?: 'SmartListViewSort', id: string, desc: boolean }>, groupColumns: Array<{ __typename?: 'SmartListViewColumn', id: string, position: number }>, groupSorting: Array<{ __typename?: 'SmartListViewSort', id: string, desc: boolean }> } };

export type UpdateSmartListViewMutationVariables = Exact<{
  originalName: Scalars['String']['input'];
  input: SaveSmartListView;
}>;


export type UpdateSmartListViewMutation = { __typename?: 'Mutation', updateSmartListView: { __typename?: 'SmartListView', id: number, listId: string, name: string, search?: string | null, enableMultiSort?: boolean | null, bookColumns: Array<{ __typename?: 'SmartListViewColumn', id: string, position: number }>, bookSorting: Array<{ __typename?: 'SmartListViewSort', id: string, desc: boolean }>, groupColumns: Array<{ __typename?: 'SmartListViewColumn', id: string, position: number }>, groupSorting: Array<{ __typename?: 'SmartListViewSort', id: string, desc: boolean }> } };

export type DeleteSmartListViewMutationVariables = Exact<{
  id: Scalars['ID']['input'];
  name: Scalars['String']['input'];
}>;


export type DeleteSmartListViewMutation = { __typename?: 'Mutation', deleteSmartListView: { __typename?: 'SmartListView', id: number, name: string } };

export type SmartListBasicSettingsSceneQueryVariables = Exact<{ [key: string]: never; }>;


export type SmartListBasicSettingsSceneQuery = { __typename?: 'Query', smartLists: Array<{ __typename?: 'SmartList', name: string }> };

export type DeleteSmartListMutationVariables = Exact<{
  id: Scalars['ID']['input'];
}>;


export type DeleteSmartListMutation = { __typename?: 'Mutation', deleteSmartList: { __typename: 'SmartList' } };

export type MyReaderSettingsQueryVariables = Exact<{ [key: string]: never; }>;


export type MyReaderSettingsQuery = { __typename?: 'Query', myReaderSettings?: string | null };

export type UpdateMyReaderSettingsMutationVariables = Exact<{
  settings: Scalars['String']['input'];
}>;


export type UpdateMyReaderSettingsMutation = { __typename?: 'Mutation', updateMyReaderSettings: boolean };

export type DirectoryListingQueryVariables = Exact<{
  input: DirectoryListingInput;
  pagination: Pagination;
}>;


export type DirectoryListingQuery = { __typename?: 'Query', listDirectory: { __typename?: 'PaginatedDirectoryListingResponse', nodes: Array<{ __typename?: 'DirectoryListing', parent?: string | null, files: Array<{ __typename?: 'DirectoryListingFile', name: string, path: string, isDirectory: boolean, media?: { __typename?: 'Media', id: string, resolvedName: string, extension: string, thumbnail: { __typename?: 'ImageRef', url: string } } | null }> }>, pageInfo: { __typename: 'CursorPaginationInfo' } | { __typename: 'OffsetPaginationInfo', currentPage: number, totalPages: number, pageSize: number, pageOffset: number, zeroBased: boolean } } };

export type UploadConfigQueryVariables = Exact<{ [key: string]: never; }>;


export type UploadConfigQuery = { __typename?: 'Query', uploadConfig: { __typename?: 'UploadConfig', enabled: boolean, maxFileUploadSize: number } };

export class TypedDocumentString<TResult, TVariables>
  extends String
  implements DocumentTypeDecoration<TResult, TVariables>
{
  __apiType?: NonNullable<DocumentTypeDecoration<TResult, TVariables>['__apiType']>;
  private value: string;
  public __meta__?: Record<string, any> | undefined;

  constructor(value: string, __meta__?: Record<string, any> | undefined) {
    super(value);
    this.value = value;
    this.__meta__ = __meta__;
  }

  override toString(): string & DocumentTypeDecoration<TResult, TVariables> {
    return this.value;
  }
}
export const ReadingNowFragmentDoc = new TypedDocumentString(`
    fragment ReadingNow on Media {
  id
  resolvedName
  metadata {
    summary
    genres
    links
    publisher
    year
  }
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
  pages
  readProgress {
    page
    percentageCompleted
    updatedAt
    locator {
      locations {
        position
      }
    }
  }
}
    `, {"fragmentName":"ReadingNow"}) as unknown as TypedDocumentString<ReadingNowFragment, unknown>;
export const BookListItemFragmentDoc = new TypedDocumentString(`
    fragment BookListItem on Media {
  id
  resolvedName
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
  pages
  readProgress {
    page
    percentageCompleted
  }
  readHistory {
    completedAt
  }
}
    `, {"fragmentName":"BookListItem"}) as unknown as TypedDocumentString<BookListItemFragment, unknown>;
export const BookSearchItemFragmentDoc = new TypedDocumentString(`
    fragment BookSearchItem on Media {
  id
  resolvedName
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
  size
  pages
}
    `, {"fragmentName":"BookSearchItem"}) as unknown as TypedDocumentString<BookSearchItemFragment, unknown>;
export const HorizontalBookListItemFragmentDoc = new TypedDocumentString(`
    fragment HorizontalBookListItem on Media {
  id
  resolvedName
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
}
    `, {"fragmentName":"HorizontalBookListItem"}) as unknown as TypedDocumentString<HorizontalBookListItemFragment, unknown>;
export const OnDeckBookItemFragmentDoc = new TypedDocumentString(`
    fragment OnDeckBookItem on Media {
  id
  metadata {
    number
  }
  resolvedName
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
  seriesPosition
  series {
    resolvedName
    mediaCount
    metadata {
      totalIssues
    }
  }
}
    `, {"fragmentName":"OnDeckBookItem"}) as unknown as TypedDocumentString<OnDeckBookItemFragment, unknown>;
export const BookMenuFragmentDoc = new TypedDocumentString(`
    fragment BookMenu on Media {
  id
  resolvedName
  isFavorite
  library {
    id
    name
  }
  series {
    id
    resolvedName
  }
  readProgress {
    __typename
  }
  readHistory {
    __typename
  }
}
    `, {"fragmentName":"BookMenu"}) as unknown as TypedDocumentString<BookMenuFragment, unknown>;
export const BookClubCardFragmentDoc = new TypedDocumentString(`
    fragment BookClubCard on BookClub {
  id
  name
  slug
  description
  membersCount
  members {
    id
    displayName
    avatarUrl
  }
  currentBook {
    id
    imageUrl
    title
    entity {
      __typename
      id
      thumbnail {
        url
        metadata {
          averageColor
          colors {
            color
            percentage
          }
          thumbhash
        }
      }
    }
  }
}
    `, {"fragmentName":"BookClubCard"}) as unknown as TypedDocumentString<BookClubCardFragment, unknown>;
export const CurrentBookCardFragmentDoc = new TypedDocumentString(`
    fragment CurrentBookCard on BookClubBook {
  id
  title
  author
  imageUrl
  addedAt
  url
  entity {
    __typename
    id
    resolvedName
    thumbnail {
      url
      metadata {
        averageColor
        colors {
          color
          percentage
        }
        thumbhash
      }
    }
  }
}
    `, {"fragmentName":"CurrentBookCard"}) as unknown as TypedDocumentString<CurrentBookCardFragment, unknown>;
export const PastBookGridItemFragmentDoc = new TypedDocumentString(`
    fragment PastBookGridItem on BookClubBook {
  id
  imageUrl
  title
  entity {
    __typename
    id
    resolvedName
    thumbnail {
      url
      metadata {
        averageColor
        colors {
          color
          percentage
        }
        thumbhash
      }
    }
  }
  completedAt
}
    `, {"fragmentName":"PastBookGridItem"}) as unknown as TypedDocumentString<PastBookGridItemFragment, unknown>;
export const PastDiscussionsLinkFragmentDoc = new TypedDocumentString(`
    fragment PastDiscussionsLink on BookClub {
  previousBook {
    imageUrl
    entity {
      __typename
      id
      thumbnail {
        url
        metadata {
          averageColor
          colors {
            color
            percentage
          }
          thumbhash
        }
      }
    }
  }
  previousDiscussionsCount
}
    `, {"fragmentName":"PastDiscussionsLink"}) as unknown as TypedDocumentString<PastDiscussionsLinkFragment, unknown>;
export const DiscussionListItemFragmentDoc = new TypedDocumentString(`
    fragment DiscussionListItem on BookClubDiscussion {
  id
  displayName
  emoji
  messageCount
  isLocked
  isArchived
}
    `, {"fragmentName":"DiscussionListItem"}) as unknown as TypedDocumentString<DiscussionListItemFragment, unknown>;
export const LibraryGridItemFragmentDoc = new TypedDocumentString(`
    fragment LibraryGridItem on Library {
  id
  name
  series(take: 5) {
    thumbnail {
      url
      metadata {
        averageColor
        colors {
          color
          percentage
        }
        thumbhash
      }
      height
      width
    }
  }
}
    `, {"fragmentName":"LibraryGridItem"}) as unknown as TypedDocumentString<LibraryGridItemFragment, unknown>;
export const LibrarySearchItemFragmentDoc = new TypedDocumentString(`
    fragment LibrarySearchItem on Library {
  id
  name
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
}
    `, {"fragmentName":"LibrarySearchItem"}) as unknown as TypedDocumentString<LibrarySearchItemFragment, unknown>;
export const DailyActivityGridFragmentDoc = new TypedDocumentString(`
    fragment DailyActivityGrid on ReadingStats {
  range {
    start
    end
  }
  dailyActivity {
    date
    totalSeconds
    sessionCount
  }
}
    `, {"fragmentName":"DailyActivityGrid"}) as unknown as TypedDocumentString<DailyActivityGridFragment, unknown>;
export const TimeToFinishStatsFragmentDoc = new TypedDocumentString(`
    fragment TimeToFinishStats on ReadingStats {
  timeToFinish {
    libraryType
    sampleSize
    medianActiveSeconds
  }
}
    `, {"fragmentName":"TimeToFinishStats"}) as unknown as TypedDocumentString<TimeToFinishStatsFragment, unknown>;
export const WeekdayChartFragmentDoc = new TypedDocumentString(`
    fragment WeekdayChart on ReadingStats {
  dayOfWeek {
    isoWeekday
    averageSeconds
  }
}
    `, {"fragmentName":"WeekdayChart"}) as unknown as TypedDocumentString<WeekdayChartFragment, unknown>;
export const ReadingSessionCardMediaFragmentDoc = new TypedDocumentString(`
    fragment ReadingSessionCardMedia on SessionWithEvents {
  session {
    media {
      resolvedName
      pages
      thumbnail {
        url
        metadata {
          averageColor
          colors {
            color
            percentage
          }
          thumbhash
        }
      }
    }
  }
}
    `, {"fragmentName":"ReadingSessionCardMedia"}) as unknown as TypedDocumentString<ReadingSessionCardMediaFragment, unknown>;
export const BookmarkEventFragmentDoc = new TypedDocumentString(`
    fragment BookmarkEvent on Bookmark {
  id
  page
  bookmarkLocator: locator {
    locations {
      position
    }
  }
  createdAt
}
    `, {"fragmentName":"BookmarkEvent"}) as unknown as TypedDocumentString<BookmarkEventFragment, unknown>;
export const AnnotationEventFragmentDoc = new TypedDocumentString(`
    fragment AnnotationEvent on MediaAnnotation {
  id
  annotationText
  locator {
    href
    type
    chapterTitle
    locations {
      position
      progression
      totalProgression
    }
    text {
      highlight
      after
      before
    }
  }
  createdAt
}
    `, {"fragmentName":"AnnotationEvent"}) as unknown as TypedDocumentString<AnnotationEventFragment, unknown>;
export const SessionWithEventsTimelineFragmentDoc = new TypedDocumentString(`
    fragment SessionWithEventsTimeline on SessionWithEvents {
  session {
    id
    mediaId
    createdAt
    startPage
    startLocator {
      locations {
        position
      }
    }
    updatedAt
    endPage
    endLocator {
      locations {
        position
      }
    }
  }
  events {
    __typename
    ... on Bookmark {
      id
      ...BookmarkEvent
    }
    ... on MediaAnnotation {
      id
      ...AnnotationEvent
    }
  }
}
    fragment AnnotationEvent on MediaAnnotation {
  id
  annotationText
  locator {
    href
    type
    chapterTitle
    locations {
      position
      progression
      totalProgression
    }
    text {
      highlight
      after
      before
    }
  }
  createdAt
}
fragment BookmarkEvent on Bookmark {
  id
  page
  bookmarkLocator: locator {
    locations {
      position
    }
  }
  createdAt
}`, {"fragmentName":"SessionWithEventsTimeline"}) as unknown as TypedDocumentString<SessionWithEventsTimelineFragment, unknown>;
export const ReadingSessionCardFragmentDoc = new TypedDocumentString(`
    fragment ReadingSessionCard on SessionWithEvents {
  session {
    id
    status
    createdAt
    updatedAt
    startPage
    endPage
    endLocator {
      locations {
        position
      }
    }
    endPercentage
    elapsedSeconds
    mediaId
    journalEntry {
      content
    }
    isLatest
  }
  events {
    __typename
    ... on Bookmark {
      id
    }
    ... on MediaAnnotation {
      id
      annotationText
    }
  }
  ...SessionWithEventsTimeline
}
    fragment AnnotationEvent on MediaAnnotation {
  id
  annotationText
  locator {
    href
    type
    chapterTitle
    locations {
      position
      progression
      totalProgression
    }
    text {
      highlight
      after
      before
    }
  }
  createdAt
}
fragment BookmarkEvent on Bookmark {
  id
  page
  bookmarkLocator: locator {
    locations {
      position
    }
  }
  createdAt
}
fragment SessionWithEventsTimeline on SessionWithEvents {
  session {
    id
    mediaId
    createdAt
    startPage
    startLocator {
      locations {
        position
      }
    }
    updatedAt
    endPage
    endLocator {
      locations {
        position
      }
    }
  }
  events {
    __typename
    ... on Bookmark {
      id
      ...BookmarkEvent
    }
    ... on MediaAnnotation {
      id
      ...AnnotationEvent
    }
  }
}`, {"fragmentName":"ReadingSessionCard"}) as unknown as TypedDocumentString<ReadingSessionCardFragment, unknown>;
export const BookReadingTimelineFragmentDoc = new TypedDocumentString(`
    fragment BookReadingTimeline on Media {
  id
  readingTimeline {
    readthroughs {
      readthroughNumber
      startedAt
      finishedAt
      sessions {
        session {
          id
          sessionDate
        }
        ...ReadingSessionCard
      }
    }
    totalElapsedSeconds
  }
  resolvedName
  pages
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
  }
}
    fragment ReadingSessionCard on SessionWithEvents {
  session {
    id
    status
    createdAt
    updatedAt
    startPage
    endPage
    endLocator {
      locations {
        position
      }
    }
    endPercentage
    elapsedSeconds
    mediaId
    journalEntry {
      content
    }
    isLatest
  }
  events {
    __typename
    ... on Bookmark {
      id
    }
    ... on MediaAnnotation {
      id
      annotationText
    }
  }
  ...SessionWithEventsTimeline
}
fragment AnnotationEvent on MediaAnnotation {
  id
  annotationText
  locator {
    href
    type
    chapterTitle
    locations {
      position
      progression
      totalProgression
    }
    text {
      highlight
      after
      before
    }
  }
  createdAt
}
fragment BookmarkEvent on Bookmark {
  id
  page
  bookmarkLocator: locator {
    locations {
      position
    }
  }
  createdAt
}
fragment SessionWithEventsTimeline on SessionWithEvents {
  session {
    id
    mediaId
    createdAt
    startPage
    startLocator {
      locations {
        position
      }
    }
    updatedAt
    endPage
    endLocator {
      locations {
        position
      }
    }
  }
  events {
    __typename
    ... on Bookmark {
      id
      ...BookmarkEvent
    }
    ... on MediaAnnotation {
      id
      ...AnnotationEvent
    }
  }
}`, {"fragmentName":"BookReadingTimeline"}) as unknown as TypedDocumentString<BookReadingTimelineFragment, unknown>;
export const SessionEventsTimelineFragmentDoc = new TypedDocumentString(`
    fragment SessionEventsTimeline on ReadingSession {
  id
  mediaId
  createdAt
  startPage
  startLocator {
    locations {
      position
    }
  }
  events(order: $eventOrder) {
    __typename
    ... on Bookmark {
      id
      ...BookmarkEvent
    }
    ... on MediaAnnotation {
      id
      ...AnnotationEvent
    }
  }
  updatedAt
  endPage
  endLocator {
    locations {
      position
    }
  }
}
    fragment AnnotationEvent on MediaAnnotation {
  id
  annotationText
  locator {
    href
    type
    chapterTitle
    locations {
      position
      progression
      totalProgression
    }
    text {
      highlight
      after
      before
    }
  }
  createdAt
}
fragment BookmarkEvent on Bookmark {
  id
  page
  bookmarkLocator: locator {
    locations {
      position
    }
  }
  createdAt
}`, {"fragmentName":"SessionEventsTimeline"}) as unknown as TypedDocumentString<SessionEventsTimelineFragment, unknown>;
export const ReadingSessionDetailHeaderFragmentDoc = new TypedDocumentString(`
    fragment ReadingSessionDetailHeader on ReadingSession {
  id
  startPage
  endPage
  startLocator {
    locations {
      position
    }
  }
  endLocator {
    locations {
      position
    }
  }
  startPercentage
  endPercentage
  createdAt
  updatedAt
  elapsedSeconds
  chaptersRead
  media {
    thumbnail {
      url
      metadata {
        averageColor
        colors {
          color
          percentage
        }
        thumbhash
      }
    }
  }
  devices {
    name
  }
}
    `, {"fragmentName":"ReadingSessionDetailHeader"}) as unknown as TypedDocumentString<ReadingSessionDetailHeaderFragment, unknown>;
export const ReadingSessionMenuFragmentDoc = new TypedDocumentString(`
    fragment ReadingSessionMenu on ReadingSession {
  id
  status
  mediaId
  createdAt
  updatedAt
  elapsedSeconds
}
    `, {"fragmentName":"ReadingSessionMenu"}) as unknown as TypedDocumentString<ReadingSessionMenuFragment, unknown>;
export const SessionJournalFragmentDoc = new TypedDocumentString(`
    fragment SessionJournal on ReadingSession {
  id
  journalEntry {
    id
    content
    updatedAt
    mediaId
  }
}
    `, {"fragmentName":"SessionJournal"}) as unknown as TypedDocumentString<SessionJournalFragment, unknown>;
export const RecentlyAddedSeriesItemFragmentDoc = new TypedDocumentString(`
    fragment RecentlyAddedSeriesItem on Series {
  id
  createdAt
  resolvedName
  stackedThumbnails {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
  mediaCount
  readCount
}
    `, {"fragmentName":"RecentlyAddedSeriesItem"}) as unknown as TypedDocumentString<RecentlyAddedSeriesItemFragment, unknown>;
export const SeriesGridItemFragmentDoc = new TypedDocumentString(`
    fragment SeriesGridItem on Series {
  id
  resolvedName
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
  isComplete
}
    `, {"fragmentName":"SeriesGridItem"}) as unknown as TypedDocumentString<SeriesGridItemFragment, unknown>;
export const SeriesListItemFragmentDoc = new TypedDocumentString(`
    fragment SeriesListItem on Series {
  id
  resolvedName
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
  isComplete
  mediaCount
  readCount
}
    `, {"fragmentName":"SeriesListItem"}) as unknown as TypedDocumentString<SeriesListItemFragment, unknown>;
export const SeriesSearchItemFragmentDoc = new TypedDocumentString(`
    fragment SeriesSearchItem on Series {
  id
  resolvedName
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
  readCount
  mediaCount
  percentageCompleted
}
    `, {"fragmentName":"SeriesSearchItem"}) as unknown as TypedDocumentString<SeriesSearchItemFragment, unknown>;
export const SmartListBookItemFragmentDoc = new TypedDocumentString(`
    fragment SmartListBookItem on Media {
  id
  resolvedName
  name
  readProgress {
    page
    percentageCompleted
    locator {
      chapterTitle
    }
  }
  pages
  size
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
}
    `, {"fragmentName":"SmartListBookItem"}) as unknown as TypedDocumentString<SmartListBookItemFragment, unknown>;
export const SmartListGridItemFragmentDoc = new TypedDocumentString(`
    fragment SmartListGridItem on SmartList {
  id
  name
  description
  books(limit: 5) {
    thumbnail {
      url
      metadata {
        averageColor
        colors {
          color
          percentage
        }
        thumbhash
      }
    }
  }
  meta {
    matchedBooks
    matchedSeries
    matchedLibraries
  }
}
    `, {"fragmentName":"SmartListGridItem"}) as unknown as TypedDocumentString<SmartListGridItemFragment, unknown>;
export const BookCardFragmentDoc = new TypedDocumentString(`
    fragment BookCard on Media {
  id
  resolvedName
  extension
  pages
  size
  status
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
  readProgress {
    percentageCompleted
    page
    updatedAt
    locator {
      href
    }
  }
  readHistory {
    __typename
    completedAt
  }
  createdAt
  libraryConfig {
    skipBookOverview
  }
  metadata {
    number
    rating
    ageRating
    language
  }
}
    `, {"fragmentName":"BookCard"}) as unknown as TypedDocumentString<BookCardFragment, unknown>;
export const BookReadingActionsFragmentDoc = new TypedDocumentString(`
    fragment BookReadingActions on Media {
  id
  extension
  readProgress {
    page
    locator {
      href
    }
  }
  readHistory {
    __typename
  }
}
    `, {"fragmentName":"BookReadingActions"}) as unknown as TypedDocumentString<BookReadingActionsFragment, unknown>;
export const BookHeaderFragmentDoc = new TypedDocumentString(`
    fragment BookHeader on Media {
  id
  resolvedName
  pages
  extension
  series {
    id
    resolvedName
  }
  library {
    id
    name
  }
  ...BookReadingActions
}
    fragment BookReadingActions on Media {
  id
  extension
  readProgress {
    page
    locator {
      href
    }
  }
  readHistory {
    __typename
  }
}`, {"fragmentName":"BookHeader"}) as unknown as TypedDocumentString<BookHeaderFragment, unknown>;
export const BookInfoSheetFragmentDoc = new TypedDocumentString(`
    fragment BookInfoSheet on Media {
  id
  resolvedName
  pages
  size
  extension
  hash
  koreaderHash
  relativeLibraryPath
  createdAt
  modifiedAt
  library {
    id
    name
  }
  series {
    id
    resolvedName
  }
  metadata {
    publisher
    year
    month
    day
    language
    ageRating
    volume
    number
    storyArc
    storyArcNumber
    writers
    pencillers
    inkers
    colorists
    letterers
    coverArtists
    editors
    characters
    teams
    identifierIsbn
    identifierAmazon
    identifierCalibre
    identifierGoogle
    identifierMobiAsin
    identifierUuid
  }
  readHistory {
    completedAt
    dnf
  }
}
    `, {"fragmentName":"BookInfoSheet"}) as unknown as TypedDocumentString<BookInfoSheetFragment, unknown>;
export const BookStatRowFragmentDoc = new TypedDocumentString(`
    fragment BookStatRow on Media {
  pages
  size
  extension
  metadata {
    year
    rating
    ageRating
    language
  }
}
    `, {"fragmentName":"BookStatRow"}) as unknown as TypedDocumentString<BookStatRowFragment, unknown>;
export const BookOverviewDetailsFragmentDoc = new TypedDocumentString(`
    fragment BookOverviewDetails on Media {
  id
  resolvedName
  seriesPosition
  series {
    id
    resolvedName
  }
  library {
    id
    name
    config {
      libraryType
    }
  }
  metadata {
    summary
    links
    genres
    writers
    pencillers
    number
    volume
  }
  tags {
    id
    name
  }
  readProgress {
    percentageCompleted
  }
  readHistory {
    __typename
  }
  ...BookStatRow
}
    fragment BookStatRow on Media {
  pages
  size
  extension
  metadata {
    year
    rating
    ageRating
    language
  }
}`, {"fragmentName":"BookOverviewDetails"}) as unknown as TypedDocumentString<BookOverviewDetailsFragment, unknown>;
export const BookSearchSelectionFragmentDoc = new TypedDocumentString(`
    fragment BookSearchSelection on Media {
  id
  resolvedName
  thumbnail {
    url
  }
}
    `, {"fragmentName":"BookSearchSelection"}) as unknown as TypedDocumentString<BookSearchSelectionFragment, unknown>;
export const SimpleBookCardFragmentDoc = new TypedDocumentString(`
    fragment SimpleBookCard on Media {
  id
  resolvedName
  createdAt
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
  }
}
    `, {"fragmentName":"SimpleBookCard"}) as unknown as TypedDocumentString<SimpleBookCardFragment, unknown>;
export const MediaMetadataEditorFragmentDoc = new TypedDocumentString(`
    fragment MediaMetadataEditor on MediaMetadata {
  ageRating
  characters
  colorists
  coverArtists
  day
  editors
  format
  identifierAmazon
  identifierCalibre
  identifierGoogle
  identifierIsbn
  identifierMobiAsin
  identifierUuid
  genres
  inkers
  language
  letterers
  links
  month
  notes
  number
  pageCount
  pencillers
  publisher
  series
  seriesGroup
  storyArc
  storyArcNumber
  summary
  teams
  title
  titleSort
  volume
  writers
  year
  lockedFields
}
    `, {"fragmentName":"MediaMetadataEditor"}) as unknown as TypedDocumentString<MediaMetadataEditorFragment, unknown>;
export const BookClubBookItemFragmentDoc = new TypedDocumentString(`
    fragment BookClubBookItem on BookClubBook {
  id
  title
  author
  imageUrl
  url
  entity {
    __typename
    id
    resolvedName
    metadata {
      writers
    }
    thumbnail {
      url
    }
  }
  completedAt
  addedAt
}
    `, {"fragmentName":"BookClubBookItem"}) as unknown as TypedDocumentString<BookClubBookItemFragment, unknown>;
export const LibraryWebGridItemFragmentDoc = new TypedDocumentString(`
    fragment LibraryWebGridItem on Library {
  id
  name
  series(take: 5) {
    thumbnail {
      url
      metadata {
        averageColor
        colors {
          color
          percentage
        }
        thumbhash
      }
    }
  }
  stats {
    seriesCount
    bookCount
  }
}
    `, {"fragmentName":"LibraryWebGridItem"}) as unknown as TypedDocumentString<LibraryWebGridItemFragment, unknown>;
export const PendingMatchRecordFragmentDoc = new TypedDocumentString(`
    fragment PendingMatchRecord on MetadataFetchRecord {
  id
  status
  mediaId
  seriesId
  rawHits
  matchCandidates {
    provider
    externalId
    metadata {
      __typename
      ... on ExternalMediaMetadata {
        title
        seriesName
        seriesExternalId
        summary
        pageCount
        number
        day
        month
        year
        genres
        tags
        isbn
        isbn13
        writers
        artists
        colorists
        letterers
        coverArtists
      }
      ... on ExternalSeriesMetadata {
        seriesTitle: title
        alternativeTitles
        summary
        volumeCount
        coverUrl
        status
        year
        endYear
        genres
        tags
        authors
        ageRating
        publisher
      }
    }
    confidence
    confidenceFactors {
      factor
      weight
      matched
    }
  }
  addedAt
  updatedAt
  media {
    id
    resolvedName
    metadata {
      title
      summary
      series
      number
      genres
      writers
      colorists
      letterers
      coverArtists
      publisher
      year
      month
      day
      pageCount
      identifierIsbn
      lockedFields
    }
  }
  series {
    id
    resolvedName
    metadata {
      title
      summary
      genres
      writers
      publisher
      year
      status
      ageRating
      volume
      lockedFields
    }
  }
}
    `, {"fragmentName":"PendingMatchRecord"}) as unknown as TypedDocumentString<PendingMatchRecordFragment, unknown>;
export const LibraryEmojiFragmentDoc = new TypedDocumentString(`
    fragment LibraryEmoji on Library {
  id
}
    `, {"fragmentName":"LibraryEmoji"}) as unknown as TypedDocumentString<LibraryEmojiFragment, unknown>;
export const LibraryOptionsMenuFragmentDoc = new TypedDocumentString(`
    fragment LibraryOptionsMenu on Library {
  id
  name
}
    `, {"fragmentName":"LibraryOptionsMenu"}) as unknown as TypedDocumentString<LibraryOptionsMenuFragment, unknown>;
export const SeriesMetadataEditorFragmentDoc = new TypedDocumentString(`
    fragment SeriesMetadataEditor on SeriesMetadata {
  ageRating
  booktype
  characters
  collects {
    series
    comicid
    issueid
    issues
  }
  comicImage
  comicid
  descriptionFormatted
  genres
  imprint
  links
  metaType
  publicationRun
  publisher
  status
  summary
  title
  totalIssues
  volume
  writers
  year
  lockedFields
}
    `, {"fragmentName":"SeriesMetadataEditor"}) as unknown as TypedDocumentString<SeriesMetadataEditorFragment, unknown>;
export const BookMetadataFragmentDoc = new TypedDocumentString(`
    fragment BookMetadata on Media {
  metadata {
    ageRating
    rating
    characters
    colorists
    coverArtists
    editors
    genres
    inkers
    letterers
    links
    pencillers
    publisher
    series
    teams
    writers
    year
    month
    day
    volume
    number
  }
}
    `, {"fragmentName":"BookMetadata"}) as unknown as TypedDocumentString<BookMetadataFragment, unknown>;
export const ContinueReadingBookFragmentDoc = new TypedDocumentString(`
    fragment ContinueReadingBook on Media {
  id
  resolvedName
  pages
  extension
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
  }
  readProgress {
    percentageCompleted
    page
    updatedAt
    locator {
      href
    }
  }
}
    `, {"fragmentName":"ContinueReadingBook"}) as unknown as TypedDocumentString<ContinueReadingBookFragment, unknown>;
export const OnDeckBookFragmentDoc = new TypedDocumentString(`
    fragment OnDeckBook on Media {
  id
  metadata {
    number
  }
  resolvedName
  seriesPosition
  series {
    mediaCount
    metadata {
      totalIssues
    }
  }
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
  }
}
    `, {"fragmentName":"OnDeckBook"}) as unknown as TypedDocumentString<OnDeckBookFragment, unknown>;
export const RecentlyAddedBookFragmentDoc = new TypedDocumentString(`
    fragment RecentlyAddedBook on Media {
  id
  resolvedName
  createdAt
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
  }
}
    `, {"fragmentName":"RecentlyAddedBook"}) as unknown as TypedDocumentString<RecentlyAddedBookFragment, unknown>;
export const LibrarySettingsConfigFragmentDoc = new TypedDocumentString(`
    fragment LibrarySettingsConfig on Library {
  config {
    id
    convertRarToZip
    hardDeleteConversions
    defaultReadingDir
    defaultReadingMode
    defaultReadingImageScaleFit
    defaultLibraryViewMode
    hideSeriesView
    skipBookOverview
    generateFileHashes
    generateKoreaderHashes
    processMetadata
    watch
    libraryPattern
    organizationMode
    libraryType
    thumbnailConfig {
      __typename
      resizeMethod {
        __typename
        ... on ScaleEvenlyByFactor {
          factor
        }
        ... on ExactDimensionResize {
          width
          height
        }
        ... on ScaledDimensionResize {
          dimension
          size
        }
      }
      format
      quality
      page
    }
    processThumbnailColorsEvenWithoutConfig
    ignoreRules
    oneshotsDirectory
  }
}
    `, {"fragmentName":"LibrarySettingsConfig"}) as unknown as TypedDocumentString<LibrarySettingsConfigFragment, unknown>;
export const ReadingListGridItemFragmentDoc = new TypedDocumentString(`
    fragment ReadingListGridItem on ReadingList {
  id
  name
  bookCount
  books(limit: 5) {
    id
    thumbnail {
      url
      metadata {
        averageColor
        colors {
          color
          percentage
        }
        thumbhash
      }
    }
  }
}
    `, {"fragmentName":"ReadingListGridItem"}) as unknown as TypedDocumentString<ReadingListGridItemFragment, unknown>;
export const CreateOrUpdateEmailerFormFragmentDoc = new TypedDocumentString(`
    fragment CreateOrUpdateEmailerForm on Emailer {
  name
  isPrimary
  smtpHost
  smtpPort
  maxAttachmentSizeBytes
  senderDisplayName
  senderEmail
  tlsEnabled
  username
}
    `, {"fragmentName":"CreateOrUpdateEmailerForm"}) as unknown as TypedDocumentString<CreateOrUpdateEmailerFormFragment, unknown>;
export const EmailerListItemFragmentDoc = new TypedDocumentString(`
    fragment EmailerListItem on Emailer {
  id
  name
  isPrimary
  smtpHost
  smtpPort
  lastUsedAt
  maxAttachmentSizeBytes
  senderDisplayName
  senderEmail
  tlsEnabled
  username
}
    `, {"fragmentName":"EmailerListItem"}) as unknown as TypedDocumentString<EmailerListItemFragment, unknown>;
export const JobDataInspectorFragmentDoc = new TypedDocumentString(`
    fragment JobDataInspector on CoreJobOutput {
  __typename
  ... on LibraryScanOutput {
    totalFiles
    totalDirectories
    ignoredFiles
    skippedFiles
    ignoredDirectories
    createdMedia
    updatedMedia
    createdSeries
    updatedSeries
  }
  ... on SeriesScanOutput {
    totalFiles
    ignoredFiles
    skippedFiles
    createdMedia
    updatedMedia
  }
  ... on ThumbnailGenerationOutput {
    visitedFiles
    skippedFiles
    generatedThumbnails
    removedThumbnails
  }
  ... on TrashPurgeOutput {
    purgedBooks
    missingBookFiles
    keptBooks
    purgedHeldFiles
    freedBytes
  }
  ... on RescanBooksOutput {
    rescannedBooks
    updatedBooks
    missingBooks
  }
}
    `, {"fragmentName":"JobDataInspector"}) as unknown as TypedDocumentString<JobDataInspectorFragment, unknown>;
export const ScheduledJobRowFragmentDoc = new TypedDocumentString(`
    fragment ScheduledJobRow on ScheduledJob {
  id
  name
  kind
  schedule
  config
  enabled
  createdAt
  lastRunAt
}
    `, {"fragmentName":"ScheduledJobRow"}) as unknown as TypedDocumentString<ScheduledJobRowFragment, unknown>;
export const EditProviderDialogFragmentDoc = new TypedDocumentString(`
    fragment EditProviderDialog on MetadataProviderConfigModel {
  id
  providerType
  enabled
  apiTokenExpiresAt
  autoApplyConfig
}
    `, {"fragmentName":"EditProviderDialog"}) as unknown as TypedDocumentString<EditProviderDialogFragment, unknown>;
export const ExistingProviderCardFragmentDoc = new TypedDocumentString(`
    fragment ExistingProviderCard on MetadataProviderConfigModel {
  id
  providerType
  enabled
  apiTokenExpiresAt
  autoApplyConfig
  createdAt
  updatedAt
  ...EditProviderDialog
}
    fragment EditProviderDialog on MetadataProviderConfigModel {
  id
  providerType
  enabled
  apiTokenExpiresAt
  autoApplyConfig
}`, {"fragmentName":"ExistingProviderCard"}) as unknown as TypedDocumentString<ExistingProviderCardFragment, unknown>;
export const SmartListWebGridItemFragmentDoc = new TypedDocumentString(`
    fragment SmartListWebGridItem on SmartList {
  id
  name
  books(limit: 5) {
    thumbnail {
      url
      metadata {
        averageColor
        colors {
          color
          percentage
        }
        thumbhash
      }
    }
  }
  meta {
    matchedBooks
    matchedSeries
    matchedLibraries
  }
}
    `, {"fragmentName":"SmartListWebGridItem"}) as unknown as TypedDocumentString<SmartListWebGridItemFragment, unknown>;
export const SmartListItemBookMetadataFragmentDoc = new TypedDocumentString(`
    fragment SmartListItemBookMetadata on Media {
  metadata {
    ageRating
    rating
    characters
    colorists
    coverArtists
    editors
    genres
    inkers
    letterers
    links
    pencillers
    publisher
    teams
    writers
    year
    month
    day
    format
    identifierAmazon
    identifierCalibre
    identifierGoogle
    identifierIsbn
    identifierMobiAsin
    identifierUuid
    language
    notes
    number
    pageCount
    series
    seriesGroup
    storyArc
    storyArcNumber
    title
    titleSort
    volume
  }
}
    `, {"fragmentName":"SmartListItemBookMetadata"}) as unknown as TypedDocumentString<SmartListItemBookMetadataFragment, unknown>;
export const CreateBookClubMobileDocument = new TypedDocumentString(`
    mutation CreateBookClubMobile($input: CreateBookClubInput!) {
  createBookClub(input: $input) {
    id
    slug
  }
}
    `) as unknown as TypedDocumentString<CreateBookClubMobileMutation, CreateBookClubMobileMutationVariables>;
export const BookClubsScreenDocument = new TypedDocumentString(`
    query BookClubsScreen {
  bookClubs {
    id
    ...BookClubCard
  }
  myBookClubInvitations {
    id
  }
}
    fragment BookClubCard on BookClub {
  id
  name
  slug
  description
  membersCount
  members {
    id
    displayName
    avatarUrl
  }
  currentBook {
    id
    imageUrl
    title
    entity {
      __typename
      id
      thumbnail {
        url
        metadata {
          averageColor
          colors {
            color
            percentage
          }
          thumbhash
        }
      }
    }
  }
}`) as unknown as TypedDocumentString<BookClubsScreenQuery, BookClubsScreenQueryVariables>;
export const BookClubInvitesScreenDocument = new TypedDocumentString(`
    query BookClubInvitesScreen {
  myBookClubInvitations {
    id
    role
    bookClubId
    bookClub {
      name
      description
      membersCount
    }
  }
}
    `) as unknown as TypedDocumentString<BookClubInvitesScreenQuery, BookClubInvitesScreenQueryVariables>;
export const RespondToBookClubInvitationDocument = new TypedDocumentString(`
    mutation RespondToBookClubInvitation($id: ID!, $accept: Boolean!) {
  respondToBookClubInvitation(id: $id, input: {accept: $accept}) {
    id
  }
}
    `) as unknown as TypedDocumentString<RespondToBookClubInvitationMutation, RespondToBookClubInvitationMutationVariables>;
export const SearchMediaDocument = new TypedDocumentString(`
    query SearchMedia($filter: MediaFilterInput!) {
  media(filter: $filter, pagination: {cursor: {limit: 10}}) {
    nodes {
      id
      ...BookSearchItem
    }
    pageInfo {
      __typename
      ... on CursorPaginationInfo {
        nextCursor
      }
    }
  }
}
    fragment BookSearchItem on Media {
  id
  resolvedName
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
  size
  pages
}`) as unknown as TypedDocumentString<SearchMediaQuery, SearchMediaQueryVariables>;
export const SearchSeriesDocument = new TypedDocumentString(`
    query SearchSeries($filter: SeriesFilterInput!) {
  series(filter: $filter, pagination: {cursor: {limit: 10}}) {
    nodes {
      id
      ...SeriesSearchItem
    }
    pageInfo {
      __typename
      ... on CursorPaginationInfo {
        nextCursor
      }
    }
  }
}
    fragment SeriesSearchItem on Series {
  id
  resolvedName
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
  readCount
  mediaCount
  percentageCompleted
}`) as unknown as TypedDocumentString<SearchSeriesQuery, SearchSeriesQueryVariables>;
export const SearchLibraryDocument = new TypedDocumentString(`
    query SearchLibrary($search: String!) {
  libraries(search: $search, pagination: {cursor: {limit: 10}}) {
    nodes {
      id
      ...LibrarySearchItem
    }
    pageInfo {
      __typename
      ... on CursorPaginationInfo {
        nextCursor
      }
    }
  }
}
    fragment LibrarySearchItem on Library {
  id
  name
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
}`) as unknown as TypedDocumentString<SearchLibraryQuery, SearchLibraryQueryVariables>;
export const BookByIdStackLayoutDocument = new TypedDocumentString(`
    query BookByIdStackLayout($bookId: ID!) {
  mediaById(id: $bookId) {
    id
    thumbnail {
      metadata {
        averageColor
        colors {
          color
          percentage
        }
        thumbhash
      }
    }
  }
}
    `) as unknown as TypedDocumentString<BookByIdStackLayoutQuery, BookByIdStackLayoutQueryVariables>;
export const BookByIdDocument = new TypedDocumentString(`
    query BookById($id: ID!) {
  mediaById(id: $id) {
    id
    ...BookMenu
    extension
    metadata {
      ageRating
      characters
      colorists
      coverArtists
      day
      editors
      identifierAmazon
      identifierCalibre
      identifierGoogle
      identifierIsbn
      identifierMobiAsin
      identifierUuid
      genres
      inkers
      language
      letterers
      links
      month
      notes
      number
      pageCount
      pencillers
      publisher
      series
      summary
      teams
      title
      titleSort
      volume
      writers
      year
    }
    pages
    readProgress {
      page
      percentageCompleted
      locator {
        chapterTitle
        locations {
          fragments
          position
          progression
          totalProgression
          cssSelector
          partialCfi
        }
        href
        title
        type
      }
      startedAt
      elapsedSeconds
      updatedAt
    }
    readHistory {
      completedAt
      elapsedSeconds
    }
    resolvedName
    series {
      id
      resolvedName
      mediaCount
      metadata {
        totalIssues
      }
    }
    library {
      id
      name
    }
    seriesPosition
    size
    thumbnail {
      url
      metadata {
        averageColor
        colors {
          color
          percentage
        }
        thumbhash
      }
      height
      width
    }
    ebook {
      toc
    }
  }
}
    fragment BookMenu on Media {
  id
  resolvedName
  isFavorite
  library {
    id
    name
  }
  series {
    id
    resolvedName
  }
  readProgress {
    __typename
  }
  readHistory {
    __typename
  }
}`) as unknown as TypedDocumentString<BookByIdQuery, BookByIdQueryVariables>;
export const BookReadScreenDocument = new TypedDocumentString(`
    query BookReadScreen($id: ID!) {
  mediaById(id: $id) {
    id
    name: resolvedName
    pages
    extension
    thumbnail {
      url
      metadata {
        averageColor
        thumbhash
        colors {
          color
          percentage
        }
      }
    }
    readProgress {
      percentageCompleted
      locator {
        chapterTitle
        href
        title
        locations {
          fragments
          progression
          position
          totalProgression
          cssSelector
          partialCfi
        }
        type
      }
      page
      elapsedSeconds
    }
    series {
      id
      resolvedName
    }
    library {
      id
      name
    }
    libraryConfig {
      defaultReadingImageScaleFit
      defaultReadingMode
      defaultReadingDir
    }
    metadata {
      writers
      publisher
      summary
    }
    analysisData {
      dimensions {
        height
        width
      }
    }
    nextInSeries(pagination: {cursor: {limit: 1}}) {
      nodes {
        id
        name: resolvedName
        thumbnail {
          url
        }
      }
    }
    ebook {
      bookmarks {
        id
        mediaId
        previewContent
        locator {
          chapterTitle
          href
          locations {
            fragments
            progression
            position
            totalProgression
            cssSelector
            partialCfi
          }
        }
        createdAt
      }
      annotations {
        id
        annotationText
        createdAt
        updatedAt
        locator {
          chapterTitle
          href
          title
          type
          locations {
            fragments
            progression
            position
            totalProgression
            cssSelector
            partialCfi
          }
          text {
            after
            before
            highlight
          }
        }
      }
      spine {
        id
        idref
        properties
        linear
      }
      toc
    }
  }
}
    `) as unknown as TypedDocumentString<BookReadScreenQuery, BookReadScreenQueryVariables>;
export const UpdateReadProgressionDocument = new TypedDocumentString(`
    mutation UpdateReadProgression($id: ID!, $input: MediaProgressInput!) {
  updateMediaProgress(id: $id, input: $input) {
    id
    updatedAt
  }
}
    `) as unknown as TypedDocumentString<UpdateReadProgressionMutation, UpdateReadProgressionMutationVariables>;
export const ResetElapsedSecondsDocument = new TypedDocumentString(`
    mutation ResetElapsedSeconds($id: ID!) {
  resetElapsedSeconds(id: $id)
}
    `) as unknown as TypedDocumentString<ResetElapsedSecondsMutation, ResetElapsedSecondsMutationVariables>;
export const CreateBookmarkMobileDocument = new TypedDocumentString(`
    mutation CreateBookmarkMobile($input: BookmarkInput!) {
  createBookmark(input: $input) {
    id
    previewContent
    mediaId
    locator {
      chapterTitle
      href
      locations {
        fragments
        progression
        position
        totalProgression
        cssSelector
        partialCfi
      }
    }
  }
}
    `) as unknown as TypedDocumentString<CreateBookmarkMobileMutation, CreateBookmarkMobileMutationVariables>;
export const DeleteBookmarkMobileDocument = new TypedDocumentString(`
    mutation DeleteBookmarkMobile($id: String!) {
  deleteBookmark(id: $id) {
    id
  }
}
    `) as unknown as TypedDocumentString<DeleteBookmarkMobileMutation, DeleteBookmarkMobileMutationVariables>;
export const CreateAnnotationMobileDocument = new TypedDocumentString(`
    mutation CreateAnnotationMobile($input: CreateAnnotationInput!) {
  createAnnotation(input: $input) {
    id
    annotationText
    createdAt
    updatedAt
    locator {
      chapterTitle
      href
      title
      type
      locations {
        fragments
        progression
        position
        totalProgression
        cssSelector
        partialCfi
      }
      text {
        after
        before
        highlight
      }
    }
  }
}
    `) as unknown as TypedDocumentString<CreateAnnotationMobileMutation, CreateAnnotationMobileMutationVariables>;
export const UpdateAnnotationMobileDocument = new TypedDocumentString(`
    mutation UpdateAnnotationMobile($input: UpdateAnnotationInput!) {
  updateAnnotation(input: $input) {
    id
    annotationText
    updatedAt
  }
}
    `) as unknown as TypedDocumentString<UpdateAnnotationMobileMutation, UpdateAnnotationMobileMutationVariables>;
export const DeleteAnnotationMobileDocument = new TypedDocumentString(`
    mutation DeleteAnnotationMobile($id: String!) {
  deleteAnnotation(id: $id) {
    id
  }
}
    `) as unknown as TypedDocumentString<DeleteAnnotationMobileMutation, DeleteAnnotationMobileMutationVariables>;
export const BookReadingTimelineSessionIdScreenDocument = new TypedDocumentString(`
    query BookReadingTimelineSessionIdScreen($sessionId: Int!, $eventOrder: OrderDirection) {
  readingSessionById(id: $sessionId) {
    id
    ...ReadingSessionMenu
    createdAt
    updatedAt
    startPage
    endPage
    endLocator {
      locations {
        position
      }
    }
    endPercentage
    elapsedSeconds
    ...SessionEventsTimeline
    media {
      resolvedName
      pages
      thumbnail {
        url
        metadata {
          averageColor
          colors {
            color
            percentage
          }
          thumbhash
        }
      }
    }
    ...ReadingSessionDetailHeader
    ...SessionJournal
  }
}
    fragment AnnotationEvent on MediaAnnotation {
  id
  annotationText
  locator {
    href
    type
    chapterTitle
    locations {
      position
      progression
      totalProgression
    }
    text {
      highlight
      after
      before
    }
  }
  createdAt
}
fragment BookmarkEvent on Bookmark {
  id
  page
  bookmarkLocator: locator {
    locations {
      position
    }
  }
  createdAt
}
fragment SessionEventsTimeline on ReadingSession {
  id
  mediaId
  createdAt
  startPage
  startLocator {
    locations {
      position
    }
  }
  events(order: $eventOrder) {
    __typename
    ... on Bookmark {
      id
      ...BookmarkEvent
    }
    ... on MediaAnnotation {
      id
      ...AnnotationEvent
    }
  }
  updatedAt
  endPage
  endLocator {
    locations {
      position
    }
  }
}
fragment ReadingSessionDetailHeader on ReadingSession {
  id
  startPage
  endPage
  startLocator {
    locations {
      position
    }
  }
  endLocator {
    locations {
      position
    }
  }
  startPercentage
  endPercentage
  createdAt
  updatedAt
  elapsedSeconds
  chaptersRead
  media {
    thumbnail {
      url
      metadata {
        averageColor
        colors {
          color
          percentage
        }
        thumbhash
      }
    }
  }
  devices {
    name
  }
}
fragment ReadingSessionMenu on ReadingSession {
  id
  status
  mediaId
  createdAt
  updatedAt
  elapsedSeconds
}
fragment SessionJournal on ReadingSession {
  id
  journalEntry {
    id
    content
    updatedAt
    mediaId
  }
}`) as unknown as TypedDocumentString<BookReadingTimelineSessionIdScreenQuery, BookReadingTimelineSessionIdScreenQueryVariables>;
export const BookReadingTimelineScreenDocument = new TypedDocumentString(`
    query BookReadingTimelineScreen($bookId: ID!) {
  mediaById(id: $bookId) {
    id
    ...BookReadingTimeline
  }
}
    fragment ReadingSessionCard on SessionWithEvents {
  session {
    id
    status
    createdAt
    updatedAt
    startPage
    endPage
    endLocator {
      locations {
        position
      }
    }
    endPercentage
    elapsedSeconds
    mediaId
    journalEntry {
      content
    }
    isLatest
  }
  events {
    __typename
    ... on Bookmark {
      id
    }
    ... on MediaAnnotation {
      id
      annotationText
    }
  }
  ...SessionWithEventsTimeline
}
fragment BookReadingTimeline on Media {
  id
  readingTimeline {
    readthroughs {
      readthroughNumber
      startedAt
      finishedAt
      sessions {
        session {
          id
          sessionDate
        }
        ...ReadingSessionCard
      }
    }
    totalElapsedSeconds
  }
  resolvedName
  pages
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
  }
}
fragment AnnotationEvent on MediaAnnotation {
  id
  annotationText
  locator {
    href
    type
    chapterTitle
    locations {
      position
      progression
      totalProgression
    }
    text {
      highlight
      after
      before
    }
  }
  createdAt
}
fragment BookmarkEvent on Bookmark {
  id
  page
  bookmarkLocator: locator {
    locations {
      position
    }
  }
  createdAt
}
fragment SessionWithEventsTimeline on SessionWithEvents {
  session {
    id
    mediaId
    createdAt
    startPage
    startLocator {
      locations {
        position
      }
    }
    updatedAt
    endPage
    endLocator {
      locations {
        position
      }
    }
  }
  events {
    __typename
    ... on Bookmark {
      id
      ...BookmarkEvent
    }
    ... on MediaAnnotation {
      id
      ...AnnotationEvent
    }
  }
}`) as unknown as TypedDocumentString<BookReadingTimelineScreenQuery, BookReadingTimelineScreenQueryVariables>;
export const BooksScreenDocument = new TypedDocumentString(`
    query BooksScreen($pagination: Pagination, $filters: MediaFilterInput, $orderBy: [MediaOrderBy!]) {
  media(pagination: $pagination, filter: $filters, orderBy: $orderBy) {
    nodes {
      id
      ...BookListItem
      thumbnail {
        metadata {
          averageColor
        }
      }
    }
    pageInfo {
      __typename
      ... on OffsetPaginationInfo {
        totalPages
        currentPage
        pageSize
        pageOffset
        zeroBased
      }
    }
  }
}
    fragment BookListItem on Media {
  id
  resolvedName
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
  pages
  readProgress {
    page
    percentageCompleted
  }
  readHistory {
    completedAt
  }
}`) as unknown as TypedDocumentString<BooksScreenQuery, BooksScreenQueryVariables>;
export const BooksScreenStatsDocument = new TypedDocumentString(`
    query BooksScreenStats {
  librariesStats {
    seriesCount
    bookCount
    totalBytes
    completedBooks
    inProgressBooks
    totalReadingTimeSeconds
  }
}
    `) as unknown as TypedDocumentString<BooksScreenStatsQuery, BooksScreenStatsQueryVariables>;
export const BookSearchScreenDocument = new TypedDocumentString(`
    query BookSearchScreen($filter: MediaFilterInput!, $pagination: Pagination!) {
  media(filter: $filter, pagination: $pagination) {
    nodes {
      id
      ...BookListItem
    }
    pageInfo {
      __typename
      ... on CursorPaginationInfo {
        currentCursor
        nextCursor
        limit
      }
    }
  }
}
    fragment BookListItem on Media {
  id
  resolvedName
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
  pages
  readProgress {
    page
    percentageCompleted
  }
  readHistory {
    completedAt
  }
}`) as unknown as TypedDocumentString<BookSearchScreenQuery, BookSearchScreenQueryVariables>;
export const BookClubContextLayoutDocument = new TypedDocumentString(`
    query BookClubContextLayout($id: ID!) {
  bookClubById(id: $id) {
    id
    membership {
      id
      role
    }
  }
}
    `) as unknown as TypedDocumentString<BookClubContextLayoutQuery, BookClubContextLayoutQueryVariables>;
export const BookClubPastDiscussionsDocument = new TypedDocumentString(`
    query BookClubPastDiscussions($bookClubId: ID!) {
  previousBookClubDiscussions(bookClubId: $bookClubId) {
    displayName
    createdAt
    book {
      id
      ...PastBookGridItem
    }
    messageCount
  }
}
    fragment PastBookGridItem on BookClubBook {
  id
  imageUrl
  title
  entity {
    __typename
    id
    resolvedName
    thumbnail {
      url
      metadata {
        averageColor
        colors {
          color
          percentage
        }
        thumbhash
      }
    }
  }
  completedAt
}`) as unknown as TypedDocumentString<BookClubPastDiscussionsQuery, BookClubPastDiscussionsQueryVariables>;
export const BookClubPastBookScreenDocument = new TypedDocumentString(`
    query BookClubPastBookScreen($bookId: ID!) {
  bookClubDiscussionByBook(bookClubBookId: $bookId) {
    id
    ...DiscussionListItem
  }
  bookClubBook(id: $bookId) {
    title
    entity {
      resolvedName
    }
  }
}
    fragment DiscussionListItem on BookClubDiscussion {
  id
  displayName
  emoji
  messageCount
  isLocked
  isArchived
}`) as unknown as TypedDocumentString<BookClubPastBookScreenQuery, BookClubPastBookScreenQueryVariables>;
export const BookClubDiscussionRoomDocument = new TypedDocumentString(`
    query BookClubDiscussionRoom($id: ID!) {
  bookClubDiscussion(id: $id) {
    id
    displayName
    isLocked
    book {
      id
      title
      author
    }
  }
}
    `) as unknown as TypedDocumentString<BookClubDiscussionRoomQuery, BookClubDiscussionRoomQueryVariables>;
export const BookClubDiscussionMessagesDocument = new TypedDocumentString(`
    query BookClubDiscussionMessages($discussionId: ID!, $pagination: CursorPagination) {
  bookClubDiscussionMessages(discussionId: $discussionId, pagination: $pagination) {
    nodes {
      id
      content
      timestamp
      editedAt
      deletedAt
      isPinnedMessage
      parentMessageId
      memberId
      threadChildrenCount
      reactions {
        emoji
        customEmojiId
        customEmojiUrl
        count
        reactedByMe
      }
      replyTo {
        id
        content
        member {
          displayName
          username
          avatarUrl
        }
      }
      member {
        id
        displayName
        avatarUrl
        username
      }
    }
    cursorInfo {
      nextCursor
      limit
    }
  }
}
    `) as unknown as TypedDocumentString<BookClubDiscussionMessagesQuery, BookClubDiscussionMessagesQueryVariables>;
export const SendDiscussionMessageDocument = new TypedDocumentString(`
    mutation SendDiscussionMessage($discussionId: ID!, $input: SendMessageInput!) {
  sendMessage(discussionId: $discussionId, input: $input) {
    id
  }
}
    `) as unknown as TypedDocumentString<SendDiscussionMessageMutation, SendDiscussionMessageMutationVariables>;
export const ToggleMessageReactionDocument = new TypedDocumentString(`
    mutation ToggleMessageReaction($messageId: ID!, $emoji: String, $customEmojiId: Int) {
  toggleReaction(
    messageId: $messageId
    emoji: $emoji
    customEmojiId: $customEmojiId
  )
}
    `) as unknown as TypedDocumentString<ToggleMessageReactionMutation, ToggleMessageReactionMutationVariables>;
export const DeleteDiscussionMessageDocument = new TypedDocumentString(`
    mutation DeleteDiscussionMessage($messageId: ID!) {
  deleteMessage(messageId: $messageId) {
    id
  }
}
    `) as unknown as TypedDocumentString<DeleteDiscussionMessageMutation, DeleteDiscussionMessageMutationVariables>;
export const ThreadParentMessageDocument = new TypedDocumentString(`
    query ThreadParentMessage($id: ID!) {
  bookClubDiscussionMessage(id: $id) {
    id
    content
    timestamp
    editedAt
    deletedAt
    isPinnedMessage
    parentMessageId
    memberId
    threadChildrenCount
    reactions {
      emoji
      customEmojiId
      customEmojiUrl
      count
      reactedByMe
    }
    replyTo {
      id
      content
      member {
        displayName
        username
        avatarUrl
      }
    }
    member {
      id
      displayName
      avatarUrl
      username
    }
  }
}
    `) as unknown as TypedDocumentString<ThreadParentMessageQuery, ThreadParentMessageQueryVariables>;
export const ThreadRepliesDocument = new TypedDocumentString(`
    query ThreadReplies($discussionId: ID!, $parentId: ID, $pagination: CursorPagination) {
  bookClubDiscussionMessages(
    discussionId: $discussionId
    parentId: $parentId
    pagination: $pagination
  ) {
    nodes {
      id
      content
      timestamp
      editedAt
      deletedAt
      isPinnedMessage
      parentMessageId
      memberId
      threadChildrenCount
      reactions {
        emoji
        customEmojiId
        count
        reactedByMe
      }
      replyTo {
        id
        content
        member {
          displayName
          username
        }
      }
      member {
        id
        displayName
        avatarUrl
        username
      }
    }
    cursorInfo {
      nextCursor
      limit
    }
  }
}
    `) as unknown as TypedDocumentString<ThreadRepliesQuery, ThreadRepliesQueryVariables>;
export const SendThreadReplyDocument = new TypedDocumentString(`
    mutation SendThreadReply($discussionId: ID!, $input: SendMessageInput!) {
  sendMessage(discussionId: $discussionId, input: $input) {
    id
  }
}
    `) as unknown as TypedDocumentString<SendThreadReplyMutation, SendThreadReplyMutationVariables>;
export const ToggleThreadMessageReactionDocument = new TypedDocumentString(`
    mutation ToggleThreadMessageReaction($messageId: ID!, $emoji: String, $customEmojiId: Int) {
  toggleReaction(
    messageId: $messageId
    emoji: $emoji
    customEmojiId: $customEmojiId
  )
}
    `) as unknown as TypedDocumentString<ToggleThreadMessageReactionMutation, ToggleThreadMessageReactionMutationVariables>;
export const DeleteThreadMessageDocument = new TypedDocumentString(`
    mutation DeleteThreadMessage($messageId: ID!) {
  deleteMessage(messageId: $messageId) {
    id
  }
}
    `) as unknown as TypedDocumentString<DeleteThreadMessageMutation, DeleteThreadMessageMutationVariables>;
export const ThreadDiscussionInfoDocument = new TypedDocumentString(`
    query ThreadDiscussionInfo($id: ID!) {
  bookClubDiscussion(id: $id) {
    id
    displayName
    isLocked
  }
}
    `) as unknown as TypedDocumentString<ThreadDiscussionInfoQuery, ThreadDiscussionInfoQueryVariables>;
export const BookClubDetailScreenDocument = new TypedDocumentString(`
    query BookClubDetailScreen($id: ID!) {
  bookClubById(id: $id) {
    id
    name
    emoji
    membership {
      id
      role
    }
    moderators {
      id
      avatarUrl
      displayName
    }
    pinnedDiscussions {
      id
      ...DiscussionListItem
    }
    currentBook {
      id
      ...CurrentBookCard
      discussions {
        id
        ...DiscussionListItem
      }
      entity {
        id
        readProgress {
          percentageCompleted
          elapsedSeconds
          startedAt
        }
        readHistory {
          __typename
          completedAt
        }
      }
    }
    ...PastDiscussionsLink
  }
}
    fragment CurrentBookCard on BookClubBook {
  id
  title
  author
  imageUrl
  addedAt
  url
  entity {
    __typename
    id
    resolvedName
    thumbnail {
      url
      metadata {
        averageColor
        colors {
          color
          percentage
        }
        thumbhash
      }
    }
  }
}
fragment PastDiscussionsLink on BookClub {
  previousBook {
    imageUrl
    entity {
      __typename
      id
      thumbnail {
        url
        metadata {
          averageColor
          colors {
            color
            percentage
          }
          thumbhash
        }
      }
    }
  }
  previousDiscussionsCount
}
fragment DiscussionListItem on BookClubDiscussion {
  id
  displayName
  emoji
  messageCount
  isLocked
  isArchived
}`) as unknown as TypedDocumentString<BookClubDetailScreenQuery, BookClubDetailScreenQueryVariables>;
export const BookClubSettingsDocument = new TypedDocumentString(`
    query BookClubSettings($id: ID!) {
  bookClubById(id: $id) {
    id
    name
    description
    isPrivate
    emoji
    membership {
      id
      role
    }
  }
}
    `) as unknown as TypedDocumentString<BookClubSettingsQuery, BookClubSettingsQueryVariables>;
export const UpdateBookClubSettingsDocument = new TypedDocumentString(`
    mutation UpdateBookClubSettings($id: ID!, $input: UpdateBookClubInput!) {
  updateBookClub(id: $id, input: $input) {
    id
  }
}
    `) as unknown as TypedDocumentString<UpdateBookClubSettingsMutation, UpdateBookClubSettingsMutationVariables>;
export const DeleteBookClubDocument = new TypedDocumentString(`
    mutation DeleteBookClub($id: ID!) {
  deleteBookClub(id: $id) {
    id
  }
}
    `) as unknown as TypedDocumentString<DeleteBookClubMutation, DeleteBookClubMutationVariables>;
export const LeaveBookClubDocument = new TypedDocumentString(`
    mutation LeaveBookClub($id: ID!) {
  leaveBookClub(bookClubId: $id) {
    id
  }
}
    `) as unknown as TypedDocumentString<LeaveBookClubMutation, LeaveBookClubMutationVariables>;
export const LibraryPathsDocument = new TypedDocumentString(`
    query LibraryPaths {
  libraries(pagination: {none: {unpaginated: true}}) {
    nodes {
      id
      name
      path
    }
  }
}
    `) as unknown as TypedDocumentString<LibraryPathsQuery, LibraryPathsQueryVariables>;
export const LibrarySeriesScreenSeriesNameDocument = new TypedDocumentString(`
    query LibrarySeriesScreenSeriesName($id: ID!) {
  libraryById(id: $id) {
    name
    stats {
      bookCount
      seriesCount
      completedBooks
      inProgressBooks
      totalReadingTimeSeconds
    }
  }
}
    `) as unknown as TypedDocumentString<LibrarySeriesScreenSeriesNameQuery, LibrarySeriesScreenSeriesNameQueryVariables>;
export const LibrarySeriesScreenDocument = new TypedDocumentString(`
    query LibrarySeriesScreen($filter: SeriesFilterInput!, $orderBy: [SeriesOrderBy!], $pagination: Pagination) {
  series(filter: $filter, orderBy: $orderBy, pagination: $pagination) {
    nodes {
      id
      ...SeriesListItem
      thumbnail {
        metadata {
          averageColor
        }
      }
    }
    pageInfo {
      __typename
      ... on CursorPaginationInfo {
        currentCursor
        nextCursor
        limit
      }
    }
  }
}
    fragment SeriesListItem on Series {
  id
  resolvedName
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
  isComplete
  mediaCount
  readCount
}`) as unknown as TypedDocumentString<LibrarySeriesScreenQuery, LibrarySeriesScreenQueryVariables>;
export const LibrariesScreenDocument = new TypedDocumentString(`
    query LibrariesScreen($pagination: Pagination) {
  libraries(pagination: $pagination) {
    nodes {
      id
      ...LibraryGridItem
    }
    pageInfo {
      __typename
      ... on CursorPaginationInfo {
        currentCursor
        nextCursor
        limit
      }
    }
  }
}
    fragment LibraryGridItem on Library {
  id
  name
  series(take: 5) {
    thumbnail {
      url
      metadata {
        averageColor
        colors {
          color
          percentage
        }
        thumbhash
      }
      height
      width
    }
  }
}`) as unknown as TypedDocumentString<LibrariesScreenQuery, LibrariesScreenQueryVariables>;
export const SeriesBooksSceneSeriesNameDocument = new TypedDocumentString(`
    query SeriesBooksSceneSeriesName($id: ID!) {
  seriesById(id: $id) {
    kind
    resolvedName
    stats {
      bookCount
      completedBooks
      inProgressBooks
      totalReadingTimeSeconds
    }
    libraryId
  }
}
    `) as unknown as TypedDocumentString<SeriesBooksSceneSeriesNameQuery, SeriesBooksSceneSeriesNameQueryVariables>;
export const SeriesBooksScreenDocument = new TypedDocumentString(`
    query SeriesBooksScreen($filter: MediaFilterInput!, $pagination: Pagination, $orderBy: [MediaOrderBy!]) {
  media(filter: $filter, pagination: $pagination, orderBy: $orderBy) {
    nodes {
      id
      ...BookListItem
      thumbnail {
        metadata {
          averageColor
        }
      }
    }
    pageInfo {
      __typename
      ... on OffsetPaginationInfo {
        totalPages
        currentPage
        pageSize
        pageOffset
        zeroBased
      }
    }
  }
}
    fragment BookListItem on Media {
  id
  resolvedName
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
  pages
  readProgress {
    page
    percentageCompleted
  }
  readHistory {
    completedAt
  }
}`) as unknown as TypedDocumentString<SeriesBooksScreenQuery, SeriesBooksScreenQueryVariables>;
export const SeriesScreenDocument = new TypedDocumentString(`
    query SeriesScreen($pagination: Pagination, $filters: SeriesFilterInput, $orderBy: [SeriesOrderBy!]) {
  series(pagination: $pagination, filter: $filters, orderBy: $orderBy) {
    nodes {
      id
      ...SeriesListItem
      thumbnail {
        metadata {
          averageColor
        }
      }
    }
    pageInfo {
      __typename
      ... on OffsetPaginationInfo {
        totalPages
        currentPage
        pageSize
        pageOffset
        zeroBased
      }
    }
  }
}
    fragment SeriesListItem on Series {
  id
  resolvedName
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
  isComplete
  mediaCount
  readCount
}`) as unknown as TypedDocumentString<SeriesScreenQuery, SeriesScreenQueryVariables>;
export const SeriesScreenStatsDocument = new TypedDocumentString(`
    query SeriesScreenStats {
  librariesStats {
    seriesCount
    bookCount
    totalBytes
    completedBooks
    inProgressBooks
    totalReadingTimeSeconds
  }
}
    `) as unknown as TypedDocumentString<SeriesScreenStatsQuery, SeriesScreenStatsQueryVariables>;
export const SmartListScreenDocument = new TypedDocumentString(`
    query SmartListScreen($id: ID!) {
  smartListById(id: $id) {
    id
    name
    description
    items {
      __typename
      ... on SmartListGrouped {
        items {
          entity {
            __typename
            ... on Series {
              id
              resolvedName
            }
            ... on Library {
              id
              name
            }
          }
          books {
            id
            thumbnail {
              url
              metadata {
                averageColor
                colors {
                  color
                  percentage
                }
                thumbhash
              }
            }
            ...SmartListBookItem
          }
        }
      }
      ... on SmartListUngrouped {
        books {
          id
          thumbnail {
            url
            metadata {
              averageColor
              colors {
                color
                percentage
              }
              thumbhash
            }
          }
          ...SmartListBookItem
        }
      }
    }
  }
}
    fragment SmartListBookItem on Media {
  id
  resolvedName
  name
  readProgress {
    page
    percentageCompleted
    locator {
      chapterTitle
    }
  }
  pages
  size
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
}`) as unknown as TypedDocumentString<SmartListScreenQuery, SmartListScreenQueryVariables>;
export const SmartListsTabListDocument = new TypedDocumentString(`
    query SmartListsTabList {
  smartLists {
    id
    ...SmartListGridItem
  }
}
    fragment SmartListGridItem on SmartList {
  id
  name
  description
  books(limit: 5) {
    thumbnail {
      url
      metadata {
        averageColor
        colors {
          color
          percentage
        }
        thumbhash
      }
    }
  }
  meta {
    matchedBooks
    matchedSeries
    matchedLibraries
  }
}`) as unknown as TypedDocumentString<SmartListsTabListQuery, SmartListsTabListQueryVariables>;
export const PullServerAnnotationsDocument = new TypedDocumentString(`
    query PullServerAnnotations($id: ID!) {
  annotationsByMediaId(id: $id) {
    id
    annotationText
    createdAt
    updatedAt
    locator {
      chapterTitle
      href
      title
      type
      locations {
        fragments
        progression
        position
        totalProgression
        cssSelector
        partialCfi
      }
      text {
        after
        before
        highlight
      }
    }
  }
}
    `) as unknown as TypedDocumentString<PullServerAnnotationsQuery, PullServerAnnotationsQueryVariables>;
export const PullServerBookmarksDocument = new TypedDocumentString(`
    query PullServerBookmarks($id: ID!) {
  bookmarksByMediaId(id: $id) {
    id
    mediaId
    previewContent
    locator {
      chapterTitle
      href
      locations {
        fragments
        progression
        position
        totalProgression
        cssSelector
        partialCfi
      }
    }
  }
}
    `) as unknown as TypedDocumentString<PullServerBookmarksQuery, PullServerBookmarksQueryVariables>;
export const PullServerAvatarDocument = new TypedDocumentString(`
    query PullServerAvatar {
  me {
    avatar {
      url
      metadata {
        averageColor
      }
      lastModified
    }
  }
}
    `) as unknown as TypedDocumentString<PullServerAvatarQuery, PullServerAvatarQueryVariables>;
export const PullServerReadProgressionDocument = new TypedDocumentString(`
    query PullServerReadProgression($filter: MediaFilterInput!) {
  media(filter: $filter, pagination: {none: {unpaginated: true}}) {
    nodes {
      id
      readProgress {
        sessionId
        page
        percentageCompleted
        updatedAt
        elapsedSeconds
        locator {
          chapterTitle
          href
          title
          type
          locations {
            fragments
            progression
            position
            totalProgression
            cssSelector
            partialCfi
          }
        }
      }
      readHistory {
        completedAt
      }
    }
  }
}
    `) as unknown as TypedDocumentString<PullServerReadProgressionQuery, PullServerReadProgressionQueryVariables>;
export const PushCreateAnnotationDocument = new TypedDocumentString(`
    mutation PushCreateAnnotation($input: CreateAnnotationInput!) {
  createAnnotation(input: $input) {
    id
  }
}
    `) as unknown as TypedDocumentString<PushCreateAnnotationMutation, PushCreateAnnotationMutationVariables>;
export const PushUpdateAnnotationDocument = new TypedDocumentString(`
    mutation PushUpdateAnnotation($input: UpdateAnnotationInput!) {
  updateAnnotation(input: $input) {
    id
  }
}
    `) as unknown as TypedDocumentString<PushUpdateAnnotationMutation, PushUpdateAnnotationMutationVariables>;
export const PushDeleteAnnotationDocument = new TypedDocumentString(`
    mutation PushDeleteAnnotation($id: String!) {
  deleteAnnotation(id: $id) {
    id
  }
}
    `) as unknown as TypedDocumentString<PushDeleteAnnotationMutation, PushDeleteAnnotationMutationVariables>;
export const PushCreateBookmarkDocument = new TypedDocumentString(`
    mutation PushCreateBookmark($input: BookmarkInput!) {
  createBookmark(input: $input) {
    id
  }
}
    `) as unknown as TypedDocumentString<PushCreateBookmarkMutation, PushCreateBookmarkMutationVariables>;
export const PushDeleteBookmarkDocument = new TypedDocumentString(`
    mutation PushDeleteBookmark($id: String!) {
  deleteBookmark(id: $id) {
    id
  }
}
    `) as unknown as TypedDocumentString<PushDeleteBookmarkMutation, PushDeleteBookmarkMutationVariables>;
export const PushLocalReadProgressionDocument = new TypedDocumentString(`
    mutation PushLocalReadProgression($id: ID!, $input: MediaProgressInput!) {
  updateMediaProgress(id: $id, input: $input) {
    id
    updatedAt
  }
}
    `) as unknown as TypedDocumentString<PushLocalReadProgressionMutation, PushLocalReadProgressionMutationVariables>;
export const ContinueReadingDocument = new TypedDocumentString(`
    query ContinueReading($pagination: Pagination) {
  keepReading(pagination: $pagination) {
    nodes {
      id
      ...HorizontalBookListItem
      ...ReadingNow
    }
    pageInfo {
      __typename
      ... on OffsetPaginationInfo {
        totalPages
        currentPage
        pageSize
        pageOffset
        zeroBased
      }
    }
  }
}
    fragment ReadingNow on Media {
  id
  resolvedName
  metadata {
    summary
    genres
    links
    publisher
    year
  }
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
  pages
  readProgress {
    page
    percentageCompleted
    updatedAt
    locator {
      locations {
        position
      }
    }
  }
}
fragment HorizontalBookListItem on Media {
  id
  resolvedName
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
}`) as unknown as TypedDocumentString<ContinueReadingQuery, ContinueReadingQueryVariables>;
export const OnDeckBooksDocument = new TypedDocumentString(`
    query OnDeckBooks($pagination: Pagination) {
  onDeck(pagination: $pagination) {
    nodes {
      id
      ...OnDeckBookItem
    }
    pageInfo {
      __typename
      ... on OffsetPaginationInfo {
        totalPages
        currentPage
        pageSize
        pageOffset
        zeroBased
      }
    }
  }
}
    fragment OnDeckBookItem on Media {
  id
  metadata {
    number
  }
  resolvedName
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
  seriesPosition
  series {
    resolvedName
    mediaCount
    metadata {
      totalIssues
    }
  }
}`) as unknown as TypedDocumentString<OnDeckBooksQuery, OnDeckBooksQueryVariables>;
export const RecentlyAddedBooksDocument = new TypedDocumentString(`
    query RecentlyAddedBooks($pagination: Pagination) {
  recentlyAddedMedia(pagination: $pagination) {
    nodes {
      id
      ...HorizontalBookListItem
    }
    pageInfo {
      __typename
      ... on CursorPaginationInfo {
        currentCursor
        nextCursor
        limit
      }
    }
  }
}
    fragment HorizontalBookListItem on Media {
  id
  resolvedName
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
}`) as unknown as TypedDocumentString<RecentlyAddedBooksQuery, RecentlyAddedBooksQueryVariables>;
export const RecentlyAddedSeriesHorizontalDocument = new TypedDocumentString(`
    query RecentlyAddedSeriesHorizontal($pagination: Pagination) {
  recentlyAddedSeries(pagination: $pagination) {
    nodes {
      id
      ...RecentlyAddedSeriesItem
    }
    pageInfo {
      __typename
      ... on CursorPaginationInfo {
        currentCursor
        nextCursor
        limit
      }
    }
  }
}
    fragment RecentlyAddedSeriesItem on Series {
  id
  createdAt
  resolvedName
  stackedThumbnails {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
  mediaCount
  readCount
}`) as unknown as TypedDocumentString<RecentlyAddedSeriesHorizontalQuery, RecentlyAddedSeriesHorizontalQueryVariables>;
export const BooksAfterCursorDocument = new TypedDocumentString(`
    query BooksAfterCursor($id: ID!, $pagination: Pagination) {
  mediaById(id: $id) {
    nextInSeries(pagination: $pagination) {
      nodes {
        id
        ...HorizontalBookListItem
      }
      pageInfo {
        __typename
        ... on CursorPaginationInfo {
          currentCursor
          nextCursor
          limit
        }
      }
    }
  }
}
    fragment HorizontalBookListItem on Media {
  id
  resolvedName
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
}`) as unknown as TypedDocumentString<BooksAfterCursorQuery, BooksAfterCursorQueryVariables>;
export const SeriesBooksListHeaderScanSeriesDocument = new TypedDocumentString(`
    mutation SeriesBooksListHeaderScanSeries($id: ID!) {
  scanSeries(id: $id)
}
    `) as unknown as TypedDocumentString<SeriesBooksListHeaderScanSeriesMutation, SeriesBooksListHeaderScanSeriesMutationVariables>;
export const BookMenuCompleteDocument = new TypedDocumentString(`
    mutation BookMenuComplete($id: ID!) {
  finishMediaProgress(id: $id)
}
    `) as unknown as TypedDocumentString<BookMenuCompleteMutation, BookMenuCompleteMutationVariables>;
export const BookMenuDeleteSessionDocument = new TypedDocumentString(`
    mutation BookMenuDeleteSession($id: ID!) {
  clearMediaProgress(id: $id)
}
    `) as unknown as TypedDocumentString<BookMenuDeleteSessionMutation, BookMenuDeleteSessionMutationVariables>;
export const BookMenuDeleteHistoryDocument = new TypedDocumentString(`
    mutation BookMenuDeleteHistory($id: ID!) {
  deleteMediaReadingHistory(id: $id)
}
    `) as unknown as TypedDocumentString<BookMenuDeleteHistoryMutation, BookMenuDeleteHistoryMutationVariables>;
export const AddBookSheetDocument = new TypedDocumentString(`
    query AddBookSheet($pagination: Pagination, $filters: MediaFilterInput) {
  media(pagination: $pagination, filter: $filters) {
    nodes {
      id
      ...BookListItem
    }
    pageInfo {
      __typename
      ... on OffsetPaginationInfo {
        totalPages
        currentPage
        pageSize
        pageOffset
        zeroBased
      }
    }
  }
}
    fragment BookListItem on Media {
  id
  resolvedName
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
  pages
  readProgress {
    page
    percentageCompleted
  }
  readHistory {
    completedAt
  }
}`) as unknown as TypedDocumentString<AddBookSheetQuery, AddBookSheetQueryVariables>;
export const AddBookToClubDocument = new TypedDocumentString(`
    mutation AddBookToClub($bookClubId: ID!, $input: AddBookToClubInput!) {
  addBookToClub(bookClubId: $bookClubId, input: $input) {
    id
  }
}
    `) as unknown as TypedDocumentString<AddBookToClubMutation, AddBookToClubMutationVariables>;
export const ArchiveCurrentBookDocument = new TypedDocumentString(`
    mutation ArchiveCurrentBook($bookClubBookId: ID!) {
  completeBook(bookClubBookId: $bookClubBookId) {
    id
  }
}
    `) as unknown as TypedDocumentString<ArchiveCurrentBookMutation, ArchiveCurrentBookMutationVariables>;
export const PreviewBookSheetDocument = new TypedDocumentString(`
    query PreviewBookSheet($id: ID!) {
  mediaById(id: $id) {
    id
    resolvedName
    thumbnail {
      url
      metadata {
        averageColor
        colors {
          color
          percentage
        }
        thumbhash
      }
    }
    metadata {
      genres
      writers
    }
    tags {
      name
    }
  }
}
    `) as unknown as TypedDocumentString<PreviewBookSheetQuery, PreviewBookSheetQueryVariables>;
export const SuggestionsPickerSheetDocument = new TypedDocumentString(`
    query SuggestionsPickerSheet($bookClubId: ID!, $status: BookClubSuggestionStatus) {
  bookClubSuggestions(bookClubId: $bookClubId, status: $status) {
    id
    title
    author
    url
    bookId
    notes
    suggestedBy {
      user {
        username
      }
    }
  }
}
    `) as unknown as TypedDocumentString<SuggestionsPickerSheetQuery, SuggestionsPickerSheetQueryVariables>;
export const UseEmojisDocument = new TypedDocumentString(`
    query useEmojis {
  customEmojis {
    id
    name
    isAnimated
    url
  }
}
    `) as unknown as TypedDocumentString<UseEmojisQuery, UseEmojisQueryVariables>;
export const LibraryOverviewSheetDocument = new TypedDocumentString(`
    query LibraryOverviewSheet($id: ID!) {
  libraryById(id: $id) {
    name
    description
    stats {
      seriesCount
      bookCount
      totalBytes
      completedBooks
      inProgressBooks
      totalReadingTimeSeconds
    }
    tags {
      name
    }
  }
}
    `) as unknown as TypedDocumentString<LibraryOverviewSheetQuery, LibraryOverviewSheetQueryVariables>;
export const LibrarySeriesListHeaderScanLibraryDocument = new TypedDocumentString(`
    mutation LibrarySeriesListHeaderScanLibrary($id: ID!) {
  scanLibrary(id: $id)
}
    `) as unknown as TypedDocumentString<LibrarySeriesListHeaderScanLibraryMutation, LibrarySeriesListHeaderScanLibraryMutationVariables>;
export const ReadingSessionConflictViewDocument = new TypedDocumentString(`
    query ReadingSessionConflictView($mediaId: ID!, $branchedSessionId: Int) {
  readingSessionConflictView(
    mediaId: $mediaId
    branchedSessionId: $branchedSessionId
  ) {
    ancestorSession {
      __typename
      id
      endPage
      endPercentage
      elapsedSeconds
      createdAt
      updatedAt
      readthroughNumber
      endLocator {
        href
        chapterTitle
        locations {
          progression
          totalProgression
        }
      }
    }
    remoteSessions {
      __typename
      id
      endPage
      endPercentage
      elapsedSeconds
      createdAt
      updatedAt
      readthroughNumber
      endLocator {
        href
        chapterTitle
        locations {
          progression
          totalProgression
        }
      }
    }
  }
}
    `) as unknown as TypedDocumentString<ReadingSessionConflictViewQuery, ReadingSessionConflictViewQueryVariables>;
export const AcceptLocalProgressDocument = new TypedDocumentString(`
    mutation AcceptLocalProgress($id: ID!, $ancestorSessionId: Int, $input: MediaProgressInput!) {
  acceptLocalProgress(
    id: $id
    ancestorSessionId: $ancestorSessionId
    input: $input
  ) {
    id
    endPage
    endPercentage
    elapsedSeconds
    updatedAt
    endLocator {
      href
      chapterTitle
      locations {
        progression
        totalProgression
      }
    }
  }
}
    `) as unknown as TypedDocumentString<AcceptLocalProgressMutation, AcceptLocalProgressMutationVariables>;
export const MyReadingStreaksDocument = new TypedDocumentString(`
    query MyReadingStreaks {
  myReadingStreaks {
    currentDays
    longestDays
  }
}
    `) as unknown as TypedDocumentString<MyReadingStreaksQuery, MyReadingStreaksQueryVariables>;
export const RangedUserReadingStatsDocument = new TypedDocumentString(`
    query RangedUserReadingStats($range: DateRangeInput) {
  myReadingStats(range: $range) {
    range {
      start
      end
    }
    totals {
      totalSeconds
      sessionCount
      finishedCount
      activeDayCount
    }
    ...DailyActivityGrid
    ...WeekdayChart
    ...TimeToFinishStats
  }
}
    fragment DailyActivityGrid on ReadingStats {
  range {
    start
    end
  }
  dailyActivity {
    date
    totalSeconds
    sessionCount
  }
}
fragment TimeToFinishStats on ReadingStats {
  timeToFinish {
    libraryType
    sampleSize
    medianActiveSeconds
  }
}
fragment WeekdayChart on ReadingStats {
  dayOfWeek {
    isoWeekday
    averageSeconds
  }
}`) as unknown as TypedDocumentString<RangedUserReadingStatsQuery, RangedUserReadingStatsQueryVariables>;
export const PatchReadingSessionEditSheetDocument = new TypedDocumentString(`
    mutation PatchReadingSessionEditSheet($sessionId: Int!, $input: PatchReadingSession!) {
  patchReadingSession(sessionId: $sessionId, input: $input) {
    id
    elapsedSeconds
  }
}
    `) as unknown as TypedDocumentString<PatchReadingSessionEditSheetMutation, PatchReadingSessionEditSheetMutationVariables>;
export const UpdateAnnotationMobileEventTimelineDocument = new TypedDocumentString(`
    mutation UpdateAnnotationMobileEventTimeline($input: UpdateAnnotationInput!) {
  updateAnnotation(input: $input) {
    id
    annotationText
    updatedAt
  }
}
    `) as unknown as TypedDocumentString<UpdateAnnotationMobileEventTimelineMutation, UpdateAnnotationMobileEventTimelineMutationVariables>;
export const DeleteAnnotationMobileEventTimelineDocument = new TypedDocumentString(`
    mutation DeleteAnnotationMobileEventTimeline($id: String!) {
  deleteAnnotation(id: $id) {
    id
  }
}
    `) as unknown as TypedDocumentString<DeleteAnnotationMobileEventTimelineMutation, DeleteAnnotationMobileEventTimelineMutationVariables>;
export const MyReadingTimelineScreenDocument = new TypedDocumentString(`
    query MyReadingTimelineScreen($pagination: CursorPagination, $order: OrderDirection) {
  myReadingTimeline(pagination: $pagination, order: $order) {
    nodes {
      mediaId
      session {
        session {
          sessionDate
        }
        ...ReadingSessionCard
        ...ReadingSessionCardMedia
      }
    }
  }
}
    fragment ReadingSessionCard on SessionWithEvents {
  session {
    id
    status
    createdAt
    updatedAt
    startPage
    endPage
    endLocator {
      locations {
        position
      }
    }
    endPercentage
    elapsedSeconds
    mediaId
    journalEntry {
      content
    }
    isLatest
  }
  events {
    __typename
    ... on Bookmark {
      id
    }
    ... on MediaAnnotation {
      id
      annotationText
    }
  }
  ...SessionWithEventsTimeline
}
fragment ReadingSessionCardMedia on SessionWithEvents {
  session {
    media {
      resolvedName
      pages
      thumbnail {
        url
        metadata {
          averageColor
          colors {
            color
            percentage
          }
          thumbhash
        }
      }
    }
  }
}
fragment AnnotationEvent on MediaAnnotation {
  id
  annotationText
  locator {
    href
    type
    chapterTitle
    locations {
      position
      progression
      totalProgression
    }
    text {
      highlight
      after
      before
    }
  }
  createdAt
}
fragment BookmarkEvent on Bookmark {
  id
  page
  bookmarkLocator: locator {
    locations {
      position
    }
  }
  createdAt
}
fragment SessionWithEventsTimeline on SessionWithEvents {
  session {
    id
    mediaId
    createdAt
    startPage
    startLocator {
      locations {
        position
      }
    }
    updatedAt
    endPage
    endLocator {
      locations {
        position
      }
    }
  }
  events {
    __typename
    ... on Bookmark {
      id
      ...BookmarkEvent
    }
    ... on MediaAnnotation {
      id
      ...AnnotationEvent
    }
  }
}`) as unknown as TypedDocumentString<MyReadingTimelineScreenQuery, MyReadingTimelineScreenQueryVariables>;
export const UpsertReadingSessionJournalEntryDocument = new TypedDocumentString(`
    mutation UpsertReadingSessionJournalEntry($sessionId: Int!, $content: String!) {
  upsertReadingSessionJournalEntry(sessionId: $sessionId, content: $content) {
    content
    updatedAt
  }
}
    `) as unknown as TypedDocumentString<UpsertReadingSessionJournalEntryMutation, UpsertReadingSessionJournalEntryMutationVariables>;
export const DeleteReadingSessionDocument = new TypedDocumentString(`
    mutation DeleteReadingSession($sessionId: Int!) {
  deleteReadingSession(sessionId: $sessionId)
}
    `) as unknown as TypedDocumentString<DeleteReadingSessionMutation, DeleteReadingSessionMutationVariables>;
export const ExtendReadingSessionDocument = new TypedDocumentString(`
    mutation ExtendReadingSession($sessionId: Int!) {
  extendReadingSession(sessionId: $sessionId) {
    updatedAt
  }
}
    `) as unknown as TypedDocumentString<ExtendReadingSessionMutation, ExtendReadingSessionMutationVariables>;
export const RecentlyAddedSeriesGridDocument = new TypedDocumentString(`
    query RecentlyAddedSeriesGrid($pagination: Pagination) {
  series(
    pagination: $pagination
    orderBy: {series: {field: CREATED_AT, direction: DESC}}
  ) {
    nodes {
      id
      ...SeriesGridItem
    }
    pageInfo {
      __typename
      ... on OffsetPaginationInfo {
        totalPages
        currentPage
        pageSize
        pageOffset
        zeroBased
      }
    }
  }
}
    fragment SeriesGridItem on Series {
  id
  resolvedName
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
  isComplete
}`) as unknown as TypedDocumentString<RecentlyAddedSeriesGridQuery, RecentlyAddedSeriesGridQueryVariables>;
export const SeriesOverviewSheetDocument = new TypedDocumentString(`
    query SeriesOverviewSheet($id: ID!) {
  seriesById(id: $id) {
    resolvedName
    metadata {
      ageRating
      booktype
      characters
      collects {
        series
        comicid
        issueid
        issues
      }
      comicImage
      comicid
      descriptionFormatted
      genres
      imprint
      links
      metaType
      publicationRun
      publisher
      status
      summary
      title
      totalIssues
      volume
      writers
      year
    }
    stats {
      bookCount
      totalBytes
      completedBooks
      inProgressBooks
      totalReadingTimeSeconds
    }
    tags {
      name
    }
  }
}
    `) as unknown as TypedDocumentString<SeriesOverviewSheetQuery, SeriesOverviewSheetQueryVariables>;
export const SeriesBooksForDownloadDocument = new TypedDocumentString(`
    query SeriesBooksForDownload($id: ID!) {
  seriesById(id: $id) {
    id
    resolvedName
    library {
      id
      name
    }
    media {
      id
      extension
      resolvedName
      metadata {
        ageRating
        characters
        colorists
        coverArtists
        day
        editors
        format
        identifierAmazon
        identifierCalibre
        identifierGoogle
        identifierIsbn
        identifierMobiAsin
        identifierUuid
        genres
        inkers
        language
        letterers
        links
        month
        notes
        number
        pageCount
        pencillers
        publisher
        series
        seriesGroup
        storyArc
        storyArcNumber
        summary
        teams
        title
        titleSort
        volume
        writers
        year
      }
      readProgress {
        page
        percentageCompleted
        elapsedSeconds
        locator {
          chapterTitle
          href
          type
          title
          locations {
            fragments
            position
            progression
            totalProgression
            cssSelector
            partialCfi
          }
        }
        updatedAt
      }
      thumbnail {
        metadata {
          averageColor
          colors {
            color
            percentage
          }
          thumbhash
        }
      }
      ebook {
        toc
      }
    }
  }
}
    `) as unknown as TypedDocumentString<SeriesBooksForDownloadQuery, SeriesBooksForDownloadQueryVariables>;
export const UseFavoriteBookDocument = new TypedDocumentString(`
    mutation UseFavoriteBook($id: ID!, $isFavorite: Boolean!) {
  favoriteMedia(id: $id, isFavorite: $isFavorite) {
    id
    isFavorite
  }
}
    `) as unknown as TypedDocumentString<UseFavoriteBookMutation, UseFavoriteBookMutationVariables>;
export const UseSyncDeviceNameDocument = new TypedDocumentString(`
    mutation UseSyncDeviceName($deviceId: String!, $deviceName: String!) {
  upsertReadingDevice(id: $deviceId, name: $deviceName) {
    id
    name
  }
}
    `) as unknown as TypedDocumentString<UseSyncDeviceNameMutation, UseSyncDeviceNameMutationVariables>;
export const TagSelectQueryDocument = new TypedDocumentString(`
    query TagSelectQuery {
  tags {
    id
    name
  }
}
    `) as unknown as TypedDocumentString<TagSelectQueryQuery, TagSelectQueryQueryVariables>;
export const BookHeaderAnalyzeDocument = new TypedDocumentString(`
    mutation BookHeaderAnalyze($id: ID!) {
  analyzeMedia(id: $id)
}
    `) as unknown as TypedDocumentString<BookHeaderAnalyzeMutation, BookHeaderAnalyzeMutationVariables>;
export const BookSearchOverlayDocument = new TypedDocumentString(`
    query BookSearchOverlay($pagination: Pagination, $filter: MediaFilterInput!) {
  media(pagination: $pagination, filter: $filter) {
    nodes {
      id
      ...BookCard
      ...BookSearchSelection
    }
    pageInfo {
      __typename
      ... on CursorPaginationInfo {
        currentCursor
        nextCursor
        limit
      }
    }
  }
}
    fragment BookCard on Media {
  id
  resolvedName
  extension
  pages
  size
  status
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
  readProgress {
    percentageCompleted
    page
    updatedAt
    locator {
      href
    }
  }
  readHistory {
    __typename
    completedAt
  }
  createdAt
  libraryConfig {
    skipBookOverview
  }
  metadata {
    number
    rating
    ageRating
    language
  }
}
fragment BookSearchSelection on Media {
  id
  resolvedName
  thumbnail {
    url
  }
}`) as unknown as TypedDocumentString<BookSearchOverlayQuery, BookSearchOverlayQueryVariables>;
export const EmailBookDropdownDeviceDocument = new TypedDocumentString(`
    query EmailBookDropdownDevice {
  emailDevices {
    id
    name
  }
}
    `) as unknown as TypedDocumentString<EmailBookDropdownDeviceQuery, EmailBookDropdownDeviceQueryVariables>;
export const SendEmailAttachmentDocument = new TypedDocumentString(`
    mutation SendEmailAttachment($id: ID!, $sendTo: [EmailerSendTo!]!) {
  sendAttachmentEmail(input: {mediaIds: [$id], sendTo: $sendTo}) {
    sentCount
    errors
  }
}
    `) as unknown as TypedDocumentString<SendEmailAttachmentMutation, SendEmailAttachmentMutationVariables>;
export const RemoveBookFromLibraryDocument = new TypedDocumentString(`
    mutation RemoveBookFromLibrary($id: ID!) {
  deleteMedia(id: $id) {
    id
  }
}
    `) as unknown as TypedDocumentString<RemoveBookFromLibraryMutation, RemoveBookFromLibraryMutationVariables>;
export const RemoveBookRetentionDocument = new TypedDocumentString(`
    query RemoveBookRetention {
  deletedBooksSettings {
    retentionDays
  }
}
    `) as unknown as TypedDocumentString<RemoveBookRetentionQuery, RemoveBookRetentionQueryVariables>;
export const MediaEditorSetLockedFieldsDocument = new TypedDocumentString(`
    mutation MediaEditorSetLockedFields($mediaId: ID!, $lockedFields: [MetadataField!]!) {
  setMediaLockedFields(mediaId: $mediaId, lockedFields: $lockedFields) {
    id
  }
}
    `) as unknown as TypedDocumentString<MediaEditorSetLockedFieldsMutation, MediaEditorSetLockedFieldsMutationVariables>;
export const MetadataVocabularyDocument = new TypedDocumentString(`
    query MetadataVocabulary($field: MetadataFacetField!, $libraryId: ID, $search: String) {
  metadataValueCounts(field: $field, libraryId: $libraryId, search: $search) {
    value
    count
  }
}
    `) as unknown as TypedDocumentString<MetadataVocabularyQuery, MetadataVocabularyQueryVariables>;
export const MediaGenreSuggestionsDocument = new TypedDocumentString(`
    query MediaGenreSuggestions($id: ID!, $online: Boolean) {
  mediaGenreSuggestions(id: $id, online: $online) {
    suggestions {
      value
      confidence
      reasons
      libraryCount
      selected
      kind
    }
    current {
      value
      kind
      confidence
      fits
      reasons
      libraryCount
    }
    similarBooks {
      mediaId
      title
      authors
      genres
      reason
    }
    subjects
    lookedUpOnline
    onlineLookups {
      source
      status
      subjects
      message
    }
  }
}
    `) as unknown as TypedDocumentString<MediaGenreSuggestionsQuery, MediaGenreSuggestionsQueryVariables>;
export const MediaDescriptionSuggestionsDocument = new TypedDocumentString(`
    query MediaDescriptionSuggestions($id: ID!) {
  mediaDescriptionSuggestions(id: $id) {
    status
    candidates {
      source
      text
      words
    }
    onlineLookups {
      source
      status
      subjects
      message
    }
    searchedTitle
    searchedAuthors
  }
}
    `) as unknown as TypedDocumentString<MediaDescriptionSuggestionsQuery, MediaDescriptionSuggestionsQueryVariables>;
export const EditMediaMetadataDocument = new TypedDocumentString(`
    mutation EditMediaMetadata($id: ID!, $input: MediaMetadataInput!) {
  editMediaMetadata(id: $id, input: $input) {
    media {
      id
    }
    file {
      status
      fileFields
      stumpOnlyFields
      message
    }
    saveId
  }
}
    `) as unknown as TypedDocumentString<EditMediaMetadataMutation, EditMediaMetadataMutationVariables>;
export const RevertMediaMetadataEditDocument = new TypedDocumentString(`
    mutation RevertMediaMetadataEdit($mediaId: ID!, $saveId: String!) {
  revertMediaMetadataEdit(mediaId: $mediaId, saveId: $saveId) {
    media {
      id
    }
    file {
      status
      fileFields
      stumpOnlyFields
      message
    }
    saveId
    skippedFields
  }
}
    `) as unknown as TypedDocumentString<RevertMediaMetadataEditMutation, RevertMediaMetadataEditMutationVariables>;
export const MediaMetadataEditsDocument = new TypedDocumentString(`
    query MediaMetadataEdits($mediaId: ID!, $limit: Int) {
  mediaMetadataEdits(mediaId: $mediaId, limit: $limit) {
    saveId
    editedAt
    editedBy
    revertsSaveId
    revertedBy
    changes {
      field
      oldValue
      newValue
      writtenToFile
    }
  }
}
    `) as unknown as TypedDocumentString<MediaMetadataEditsQuery, MediaMetadataEditsQueryVariables>;
export const EditBookDetailsDocument = new TypedDocumentString(`
    query EditBookDetails($id: ID!) {
  mediaById(id: $id) {
    id
    resolvedName
    extension
    library {
      id
    }
    metadata {
      title
      writers
      series
      number
      genres
      summary
      publisher
      year
      month
      day
      language
      ageRating
    }
  }
}
    `) as unknown as TypedDocumentString<EditBookDetailsQuery, EditBookDetailsQueryVariables>;
export const BookOverviewSceneDocument = new TypedDocumentString(`
    query BookOverviewScene($id: ID!) {
  mediaById(id: $id) {
    id
    resolvedName
    seriesId
    metadata {
      writers
    }
    ...BookHeader
    ...BookOverviewDetails
    ...BookInfoSheet
    ...BookReadingActions
    library {
      config {
        libraryType
      }
    }
    thumbnail {
      url
      metadata {
        averageColor
        colors {
          color
          percentage
        }
        thumbhash
      }
    }
  }
}
    fragment BookHeader on Media {
  id
  resolvedName
  pages
  extension
  series {
    id
    resolvedName
  }
  library {
    id
    name
  }
  ...BookReadingActions
}
fragment BookInfoSheet on Media {
  id
  resolvedName
  pages
  size
  extension
  hash
  koreaderHash
  relativeLibraryPath
  createdAt
  modifiedAt
  library {
    id
    name
  }
  series {
    id
    resolvedName
  }
  metadata {
    publisher
    year
    month
    day
    language
    ageRating
    volume
    number
    storyArc
    storyArcNumber
    writers
    pencillers
    inkers
    colorists
    letterers
    coverArtists
    editors
    characters
    teams
    identifierIsbn
    identifierAmazon
    identifierCalibre
    identifierGoogle
    identifierMobiAsin
    identifierUuid
  }
  readHistory {
    completedAt
    dnf
  }
}
fragment BookOverviewDetails on Media {
  id
  resolvedName
  seriesPosition
  series {
    id
    resolvedName
  }
  library {
    id
    name
    config {
      libraryType
    }
  }
  metadata {
    summary
    links
    genres
    writers
    pencillers
    number
    volume
  }
  tags {
    id
    name
  }
  readProgress {
    percentageCompleted
  }
  readHistory {
    __typename
  }
  ...BookStatRow
}
fragment BookStatRow on Media {
  pages
  size
  extension
  metadata {
    year
    rating
    ageRating
    language
  }
}
fragment BookReadingActions on Media {
  id
  extension
  readProgress {
    page
    locator {
      href
    }
  }
  readHistory {
    __typename
  }
}`) as unknown as TypedDocumentString<BookOverviewSceneQuery, BookOverviewSceneQueryVariables>;
export const BookReadingActionsCompleteDocument = new TypedDocumentString(`
    mutation BookReadingActionsComplete($id: ID!) {
  finishMediaProgress(id: $id)
}
    `) as unknown as TypedDocumentString<BookReadingActionsCompleteMutation, BookReadingActionsCompleteMutationVariables>;
export const BookReadingActionsDeleteSessionDocument = new TypedDocumentString(`
    mutation BookReadingActionsDeleteSession($id: ID!) {
  clearMediaProgress(id: $id)
}
    `) as unknown as TypedDocumentString<BookReadingActionsDeleteSessionMutation, BookReadingActionsDeleteSessionMutationVariables>;
export const BookReadingActionsDeleteHistoryDocument = new TypedDocumentString(`
    mutation BookReadingActionsDeleteHistory($id: ID!) {
  deleteMediaReadingHistory(id: $id)
}
    `) as unknown as TypedDocumentString<BookReadingActionsDeleteHistoryMutation, BookReadingActionsDeleteHistoryMutationVariables>;
export const BookThumbnailSelectorUpdateDocument = new TypedDocumentString(`
    mutation BookThumbnailSelectorUpdate($id: ID!, $input: PageBasedThumbnailInput!) {
  updateMediaThumbnail(id: $id, input: $input) {
    id
    thumbnail {
      url
    }
  }
}
    `) as unknown as TypedDocumentString<BookThumbnailSelectorUpdateMutation, BookThumbnailSelectorUpdateMutationVariables>;
export const BookThumbnailSelectorUploadDocument = new TypedDocumentString(`
    mutation BookThumbnailSelectorUpload($id: ID!, $file: Upload!) {
  uploadMediaThumbnail(id: $id, file: $file) {
    id
    thumbnail {
      url
    }
  }
}
    `) as unknown as TypedDocumentString<BookThumbnailSelectorUploadMutation, BookThumbnailSelectorUploadMutationVariables>;
export const DeleteBookClubConfirmationDocument = new TypedDocumentString(`
    mutation DeleteBookClubConfirmation($id: ID!) {
  deleteBookClub(id: $id) {
    id
  }
}
    `) as unknown as TypedDocumentString<DeleteBookClubConfirmationMutation, DeleteBookClubConfirmationMutationVariables>;
export const BookClubBooksSceneDocument = new TypedDocumentString(`
    query BookClubBooksScene($id: ID!) {
  bookClubById(id: $id) {
    id
    previousBooks {
      id
      ...BookClubBookItem
    }
  }
}
    fragment BookClubBookItem on BookClubBook {
  id
  title
  author
  imageUrl
  url
  entity {
    __typename
    id
    resolvedName
    metadata {
      writers
    }
    thumbnail {
      url
    }
  }
  completedAt
  addedAt
}`) as unknown as TypedDocumentString<BookClubBooksSceneQuery, BookClubBooksSceneQueryVariables>;
export const MediaAtPathDocument = new TypedDocumentString(`
    query MediaAtPath($path: String!) {
  mediaByPath(path: $path) {
    id
    resolvedName
    thumbnail {
      url
    }
  }
}
    `) as unknown as TypedDocumentString<MediaAtPathQuery, MediaAtPathQueryVariables>;
export const UploadLibraryBooksDocument = new TypedDocumentString(`
    mutation UploadLibraryBooks($input: UploadBooksInput!) {
  uploadBooks(input: $input)
}
    `) as unknown as TypedDocumentString<UploadLibraryBooksMutation, UploadLibraryBooksMutationVariables>;
export const UploadLibrarySeriesDocument = new TypedDocumentString(`
    mutation UploadLibrarySeries($input: UploadSeriesInput!) {
  uploadSeries(input: $input)
}
    `) as unknown as TypedDocumentString<UploadLibrarySeriesMutation, UploadLibrarySeriesMutationVariables>;
export const MediaFilterFormDocument = new TypedDocumentString(`
    query MediaFilterForm($seriesId: ID, $smartListId: ID) {
  mediaMetadataOverview(seriesId: $seriesId, smartListId: $smartListId) {
    genres
    languages
    writers
    pencillers
    colorists
    letterers
    inkers
    publishers
    editors
    characters
  }
  tags {
    id
    name
  }
}
    `) as unknown as TypedDocumentString<MediaFilterFormQuery, MediaFilterFormQueryVariables>;
export const DeleteLibraryDocument = new TypedDocumentString(`
    mutation DeleteLibrary($id: ID!) {
  deleteLibrary(id: $id) {
    id
  }
}
    `) as unknown as TypedDocumentString<DeleteLibraryMutation, DeleteLibraryMutationVariables>;
export const LastVisitedLibraryDocument = new TypedDocumentString(`
    query LastVisitedLibrary {
  lastVisitedLibrary {
    id
    name
    thumbnail {
      url
    }
  }
}
    `) as unknown as TypedDocumentString<LastVisitedLibraryQuery, LastVisitedLibraryQueryVariables>;
export const LibraryBooksAlphabetDocument = new TypedDocumentString(`
    query LibraryBooksAlphabet($id: ID!) {
  libraryById(id: $id) {
    mediaAlphabet
  }
}
    `) as unknown as TypedDocumentString<LibraryBooksAlphabetQuery, LibraryBooksAlphabetQueryVariables>;
export const LibrarySeriesAlphabetDocument = new TypedDocumentString(`
    query LibrarySeriesAlphabet($id: ID!) {
  libraryById(id: $id) {
    seriesAlphabet
  }
}
    `) as unknown as TypedDocumentString<LibrarySeriesAlphabetQuery, LibrarySeriesAlphabetQueryVariables>;
export const PendingMetadataMatchesDocument = new TypedDocumentString(`
    query PendingMetadataMatches {
  pendingMetadataMatches {
    ...PendingMatchRecord
  }
}
    fragment PendingMatchRecord on MetadataFetchRecord {
  id
  status
  mediaId
  seriesId
  rawHits
  matchCandidates {
    provider
    externalId
    metadata {
      __typename
      ... on ExternalMediaMetadata {
        title
        seriesName
        seriesExternalId
        summary
        pageCount
        number
        day
        month
        year
        genres
        tags
        isbn
        isbn13
        writers
        artists
        colorists
        letterers
        coverArtists
      }
      ... on ExternalSeriesMetadata {
        seriesTitle: title
        alternativeTitles
        summary
        volumeCount
        coverUrl
        status
        year
        endYear
        genres
        tags
        authors
        ageRating
        publisher
      }
    }
    confidence
    confidenceFactors {
      factor
      weight
      matched
    }
  }
  addedAt
  updatedAt
  media {
    id
    resolvedName
    metadata {
      title
      summary
      series
      number
      genres
      writers
      colorists
      letterers
      coverArtists
      publisher
      year
      month
      day
      pageCount
      identifierIsbn
      lockedFields
    }
  }
  series {
    id
    resolvedName
    metadata {
      title
      summary
      genres
      writers
      publisher
      year
      status
      ageRating
      volume
      lockedFields
    }
  }
}`) as unknown as TypedDocumentString<PendingMetadataMatchesQuery, PendingMetadataMatchesQueryVariables>;
export const AcceptAllPendingMatchesDocument = new TypedDocumentString(`
    mutation AcceptAllPendingMatches($strategy: MergeStrategy, $excludeFields: [MetadataField!]) {
  acceptAllPendingMatches(strategy: $strategy, excludeFields: $excludeFields)
}
    `) as unknown as TypedDocumentString<AcceptAllPendingMatchesMutation, AcceptAllPendingMatchesMutationVariables>;
export const RejectAllPendingMatchesDocument = new TypedDocumentString(`
    mutation RejectAllPendingMatches {
  rejectAllPendingMatches
}
    `) as unknown as TypedDocumentString<RejectAllPendingMatchesMutation, RejectAllPendingMatchesMutationVariables>;
export const AcceptMediaMatchDocument = new TypedDocumentString(`
    mutation AcceptMediaMatch($mediaId: ID!, $candidateIndex: Int!, $strategy: MergeStrategy, $excludeFields: [MetadataField!], $overrides: [MetadataFieldOverride!]) {
  acceptMediaMatch(
    mediaId: $mediaId
    candidateIndex: $candidateIndex
    strategy: $strategy
    excludeFields: $excludeFields
    overrides: $overrides
  ) {
    ...PendingMatchRecord
  }
}
    fragment PendingMatchRecord on MetadataFetchRecord {
  id
  status
  mediaId
  seriesId
  rawHits
  matchCandidates {
    provider
    externalId
    metadata {
      __typename
      ... on ExternalMediaMetadata {
        title
        seriesName
        seriesExternalId
        summary
        pageCount
        number
        day
        month
        year
        genres
        tags
        isbn
        isbn13
        writers
        artists
        colorists
        letterers
        coverArtists
      }
      ... on ExternalSeriesMetadata {
        seriesTitle: title
        alternativeTitles
        summary
        volumeCount
        coverUrl
        status
        year
        endYear
        genres
        tags
        authors
        ageRating
        publisher
      }
    }
    confidence
    confidenceFactors {
      factor
      weight
      matched
    }
  }
  addedAt
  updatedAt
  media {
    id
    resolvedName
    metadata {
      title
      summary
      series
      number
      genres
      writers
      colorists
      letterers
      coverArtists
      publisher
      year
      month
      day
      pageCount
      identifierIsbn
      lockedFields
    }
  }
  series {
    id
    resolvedName
    metadata {
      title
      summary
      genres
      writers
      publisher
      year
      status
      ageRating
      volume
      lockedFields
    }
  }
}`) as unknown as TypedDocumentString<AcceptMediaMatchMutation, AcceptMediaMatchMutationVariables>;
export const AcceptSeriesMatchDocument = new TypedDocumentString(`
    mutation AcceptSeriesMatch($seriesId: ID!, $candidateIndex: Int!, $strategy: MergeStrategy, $excludeFields: [MetadataField!], $overrides: [MetadataFieldOverride!]) {
  acceptSeriesMatch(
    seriesId: $seriesId
    candidateIndex: $candidateIndex
    strategy: $strategy
    excludeFields: $excludeFields
    overrides: $overrides
  ) {
    ...PendingMatchRecord
  }
}
    fragment PendingMatchRecord on MetadataFetchRecord {
  id
  status
  mediaId
  seriesId
  rawHits
  matchCandidates {
    provider
    externalId
    metadata {
      __typename
      ... on ExternalMediaMetadata {
        title
        seriesName
        seriesExternalId
        summary
        pageCount
        number
        day
        month
        year
        genres
        tags
        isbn
        isbn13
        writers
        artists
        colorists
        letterers
        coverArtists
      }
      ... on ExternalSeriesMetadata {
        seriesTitle: title
        alternativeTitles
        summary
        volumeCount
        coverUrl
        status
        year
        endYear
        genres
        tags
        authors
        ageRating
        publisher
      }
    }
    confidence
    confidenceFactors {
      factor
      weight
      matched
    }
  }
  addedAt
  updatedAt
  media {
    id
    resolvedName
    metadata {
      title
      summary
      series
      number
      genres
      writers
      colorists
      letterers
      coverArtists
      publisher
      year
      month
      day
      pageCount
      identifierIsbn
      lockedFields
    }
  }
  series {
    id
    resolvedName
    metadata {
      title
      summary
      genres
      writers
      publisher
      year
      status
      ageRating
      volume
      lockedFields
    }
  }
}`) as unknown as TypedDocumentString<AcceptSeriesMatchMutation, AcceptSeriesMatchMutationVariables>;
export const RejectMediaMatchDocument = new TypedDocumentString(`
    mutation RejectMediaMatch($mediaId: ID!, $candidateIndex: Int!) {
  rejectMediaMatch(mediaId: $mediaId, candidateIndex: $candidateIndex) {
    ...PendingMatchRecord
  }
}
    fragment PendingMatchRecord on MetadataFetchRecord {
  id
  status
  mediaId
  seriesId
  rawHits
  matchCandidates {
    provider
    externalId
    metadata {
      __typename
      ... on ExternalMediaMetadata {
        title
        seriesName
        seriesExternalId
        summary
        pageCount
        number
        day
        month
        year
        genres
        tags
        isbn
        isbn13
        writers
        artists
        colorists
        letterers
        coverArtists
      }
      ... on ExternalSeriesMetadata {
        seriesTitle: title
        alternativeTitles
        summary
        volumeCount
        coverUrl
        status
        year
        endYear
        genres
        tags
        authors
        ageRating
        publisher
      }
    }
    confidence
    confidenceFactors {
      factor
      weight
      matched
    }
  }
  addedAt
  updatedAt
  media {
    id
    resolvedName
    metadata {
      title
      summary
      series
      number
      genres
      writers
      colorists
      letterers
      coverArtists
      publisher
      year
      month
      day
      pageCount
      identifierIsbn
      lockedFields
    }
  }
  series {
    id
    resolvedName
    metadata {
      title
      summary
      genres
      writers
      publisher
      year
      status
      ageRating
      volume
      lockedFields
    }
  }
}`) as unknown as TypedDocumentString<RejectMediaMatchMutation, RejectMediaMatchMutationVariables>;
export const RejectSeriesMatchDocument = new TypedDocumentString(`
    mutation RejectSeriesMatch($seriesId: ID!, $candidateIndex: Int!) {
  rejectSeriesMatch(seriesId: $seriesId, candidateIndex: $candidateIndex) {
    ...PendingMatchRecord
  }
}
    fragment PendingMatchRecord on MetadataFetchRecord {
  id
  status
  mediaId
  seriesId
  rawHits
  matchCandidates {
    provider
    externalId
    metadata {
      __typename
      ... on ExternalMediaMetadata {
        title
        seriesName
        seriesExternalId
        summary
        pageCount
        number
        day
        month
        year
        genres
        tags
        isbn
        isbn13
        writers
        artists
        colorists
        letterers
        coverArtists
      }
      ... on ExternalSeriesMetadata {
        seriesTitle: title
        alternativeTitles
        summary
        volumeCount
        coverUrl
        status
        year
        endYear
        genres
        tags
        authors
        ageRating
        publisher
      }
    }
    confidence
    confidenceFactors {
      factor
      weight
      matched
    }
  }
  addedAt
  updatedAt
  media {
    id
    resolvedName
    metadata {
      title
      summary
      series
      number
      genres
      writers
      colorists
      letterers
      coverArtists
      publisher
      year
      month
      day
      pageCount
      identifierIsbn
      lockedFields
    }
  }
  series {
    id
    resolvedName
    metadata {
      title
      summary
      genres
      writers
      publisher
      year
      status
      ageRating
      volume
      lockedFields
    }
  }
}`) as unknown as TypedDocumentString<RejectSeriesMatchMutation, RejectSeriesMatchMutationVariables>;
export const SetMediaLockedFieldsDocument = new TypedDocumentString(`
    mutation SetMediaLockedFields($mediaId: ID!, $lockedFields: [MetadataField!]!) {
  setMediaLockedFields(mediaId: $mediaId, lockedFields: $lockedFields) {
    id
  }
}
    `) as unknown as TypedDocumentString<SetMediaLockedFieldsMutation, SetMediaLockedFieldsMutationVariables>;
export const SetSeriesLockedFieldsDocument = new TypedDocumentString(`
    mutation SetSeriesLockedFields($seriesId: ID!, $lockedFields: [MetadataField!]!) {
  setSeriesLockedFields(seriesId: $seriesId, lockedFields: $lockedFields) {
    id
  }
}
    `) as unknown as TypedDocumentString<SetSeriesLockedFieldsMutation, SetSeriesLockedFieldsMutationVariables>;
export const SideBarQueryDocument = new TypedDocumentString(`
    query SideBarQuery {
  me {
    id
    preferences {
      navigationArrangement {
        locked
        sections {
          config {
            __typename
            ... on SystemArrangementConfig {
              variant
              links
            }
          }
          visible
        }
      }
    }
  }
}
    `) as unknown as TypedDocumentString<SideBarQueryQuery, SideBarQueryQueryVariables>;
export const BookClubSideBarSectionDocument = new TypedDocumentString(`
    query BookClubSideBarSection {
  bookClubs {
    id
    name
    slug
    emoji
    members {
      id
      userId
      role
    }
  }
}
    `) as unknown as TypedDocumentString<BookClubSideBarSectionQuery, BookClubSideBarSectionQueryVariables>;
export const UpdateLibraryEmojiDocument = new TypedDocumentString(`
    mutation UpdateLibraryEmoji($id: ID!, $emoji: String) {
  updateLibraryEmoji(id: $id, emoji: $emoji) {
    id
  }
}
    `) as unknown as TypedDocumentString<UpdateLibraryEmojiMutation, UpdateLibraryEmojiMutationVariables>;
export const ScanLibraryMutationDocument = new TypedDocumentString(`
    mutation ScanLibraryMutation($id: ID!) {
  scanLibrary(id: $id)
}
    `) as unknown as TypedDocumentString<ScanLibraryMutationMutation, ScanLibraryMutationMutationVariables>;
export const LibrarySideBarSectionDocument = new TypedDocumentString(`
    query LibrarySideBarSection {
  libraries(pagination: {none: {unpaginated: true}}) {
    nodes {
      id
      name
      emoji
      ...LibraryEmoji
      ...LibraryOptionsMenu
    }
  }
}
    fragment LibraryEmoji on Library {
  id
}
fragment LibraryOptionsMenu on Library {
  id
  name
}`) as unknown as TypedDocumentString<LibrarySideBarSectionQuery, LibrarySideBarSectionQueryVariables>;
export const SmartListSideBarSectionDocument = new TypedDocumentString(`
    query SmartListSideBarSection {
  smartLists {
    id
    name
  }
}
    `) as unknown as TypedDocumentString<SmartListSideBarSectionQuery, SmartListSideBarSectionQueryVariables>;
export const NavigationEntityLibraryDocument = new TypedDocumentString(`
    query NavigationEntityLibrary($id: ID!, $isSeries: Boolean!, $isBook: Boolean!) {
  seriesById(id: $id) @include(if: $isSeries) {
    libraryId
  }
  mediaById(id: $id) @include(if: $isBook) {
    libraryId
  }
}
    `) as unknown as TypedDocumentString<NavigationEntityLibraryQuery, NavigationEntityLibraryQueryVariables>;
export const PostProcessingTasksDocument = new TypedDocumentString(`
    query PostProcessingTasks($libraryId: ID, $limit: Int) {
  postProcessingTasks(libraryId: $libraryId, limit: $limit) {
    id
    libraryId
    mediaId
    kind
    steps
    status
    path
    reason
    bytesBefore
    bytesAfter
    updatedAt
  }
}
    `) as unknown as TypedDocumentString<PostProcessingTasksQuery, PostProcessingTasksQueryVariables>;
export const CreateReaderAnnotationDocument = new TypedDocumentString(`
    mutation CreateReaderAnnotation($input: CreateAnnotationInput!) {
  createAnnotation(input: $input) {
    id
    mediaId
    userId
    annotationText
    style
    createdAt
    updatedAt
    locator {
      chapterTitle
      href
      title
      type
      locations {
        fragments
        progression
        position
        totalProgression
        cssSelector
        rects {
          x
          y
          width
          height
        }
      }
      text {
        after
        before
        highlight
      }
    }
  }
}
    `) as unknown as TypedDocumentString<CreateReaderAnnotationMutation, CreateReaderAnnotationMutationVariables>;
export const UpdateReaderAnnotationDocument = new TypedDocumentString(`
    mutation UpdateReaderAnnotation($input: UpdateAnnotationInput!) {
  updateAnnotation(input: $input) {
    id
    annotationText
    style
    updatedAt
  }
}
    `) as unknown as TypedDocumentString<UpdateReaderAnnotationMutation, UpdateReaderAnnotationMutationVariables>;
export const DeleteReaderAnnotationDocument = new TypedDocumentString(`
    mutation DeleteReaderAnnotation($id: String!) {
  deleteAnnotation(id: $id) {
    id
  }
}
    `) as unknown as TypedDocumentString<DeleteReaderAnnotationMutation, DeleteReaderAnnotationMutationVariables>;
export const CreateBookmarkDocument = new TypedDocumentString(`
    mutation CreateBookmark($input: BookmarkInput!) {
  createBookmark(input: $input) {
    __typename
  }
}
    `) as unknown as TypedDocumentString<CreateBookmarkMutation, CreateBookmarkMutationVariables>;
export const DeleteBookmarkDocument = new TypedDocumentString(`
    mutation DeleteBookmark($id: String!) {
  deleteBookmark(id: $id) {
    __typename
  }
}
    `) as unknown as TypedDocumentString<DeleteBookmarkMutation, DeleteBookmarkMutationVariables>;
export const ReadiumWebReaderDocument = new TypedDocumentString(`
    query ReadiumWebReader($id: ID!) {
  epubById(id: $id) {
    mediaId
    toc
    bookmarks {
      id
      userId
      mediaId
      previewContent
      createdAt
      locator {
        chapterTitle
        href
        title
        type
        locations {
          fragments
          progression
          position
          totalProgression
          cssSelector
        }
        text {
          after
          before
          highlight
        }
      }
    }
    annotations {
      id
      mediaId
      userId
      annotationText
      style
      createdAt
      updatedAt
      locator {
        chapterTitle
        href
        title
        type
        locations {
          fragments
          progression
          position
          totalProgression
          cssSelector
        }
        text {
          after
          before
          highlight
        }
      }
    }
    media {
      id
      resolvedName
      pages
      extension
      readProgress {
        percentageCompleted
        page
        elapsedSeconds
        locator {
          chapterTitle
          href
          title
          type
          locations {
            fragments
            progression
            position
            totalProgression
            cssSelector
          }
          text {
            after
            before
            highlight
          }
        }
      }
      libraryConfig {
        defaultReadingImageScaleFit
        defaultReadingMode
        defaultReadingDir
      }
      nextInSeries(pagination: {cursor: {limit: 1}}) {
        nodes {
          id
          name: resolvedName
          thumbnail {
            url
          }
        }
      }
    }
  }
}
    `) as unknown as TypedDocumentString<ReadiumWebReaderQuery, ReadiumWebReaderQueryVariables>;
export const UpdateReadiumEpubProgressDocument = new TypedDocumentString(`
    mutation UpdateReadiumEpubProgress($id: ID!, $input: MediaProgressInput!) {
  updateMediaProgress(id: $id, input: $input) {
    __typename
  }
}
    `) as unknown as TypedDocumentString<UpdateReadiumEpubProgressMutation, UpdateReadiumEpubProgressMutationVariables>;
export const PdfReaderAnnotationsDocument = new TypedDocumentString(`
    query PdfReaderAnnotations($id: ID!) {
  annotationsByMediaId(id: $id) {
    id
    mediaId
    userId
    annotationText
    style
    createdAt
    updatedAt
    locator {
      chapterTitle
      href
      title
      type
      locations {
        fragments
        progression
        position
        totalProgression
        cssSelector
        rects {
          x
          y
          width
          height
        }
      }
      text {
        after
        before
        highlight
      }
    }
  }
}
    `) as unknown as TypedDocumentString<PdfReaderAnnotationsQuery, PdfReaderAnnotationsQueryVariables>;
export const PdfReaderBookmarksDocument = new TypedDocumentString(`
    query PdfReaderBookmarks($id: ID!) {
  bookmarksByMediaId(id: $id) {
    id
    page
    previewContent
    createdAt
    locator {
      chapterTitle
      href
      title
      type
      locations {
        fragments
        progression
        position
        totalProgression
      }
    }
  }
}
    `) as unknown as TypedDocumentString<PdfReaderBookmarksQuery, PdfReaderBookmarksQueryVariables>;
export const CreatePdfBookmarkDocument = new TypedDocumentString(`
    mutation CreatePdfBookmark($input: BookmarkInput!) {
  createBookmark(input: $input) {
    id
  }
}
    `) as unknown as TypedDocumentString<CreatePdfBookmarkMutation, CreatePdfBookmarkMutationVariables>;
export const DeletePdfBookmarkDocument = new TypedDocumentString(`
    mutation DeletePdfBookmark($id: String!) {
  deleteBookmark(id: $id) {
    id
  }
}
    `) as unknown as TypedDocumentString<DeletePdfBookmarkMutation, DeletePdfBookmarkMutationVariables>;
export const BookReadingListIdsDocument = new TypedDocumentString(`
    query BookReadingListIds($id: ID!) {
  mediaById(id: $id) {
    id
    readingListIds
  }
}
    `) as unknown as TypedDocumentString<BookReadingListIdsQuery, BookReadingListIdsQueryVariables>;
export const PickerReadingListsDocument = new TypedDocumentString(`
    query PickerReadingLists {
  readingLists(pagination: {none: {unpaginated: true}}) {
    nodes {
      id
      name
      creatingUserId
      bookCount
    }
  }
}
    `) as unknown as TypedDocumentString<PickerReadingListsQuery, PickerReadingListsQueryVariables>;
export const CreateReadingListDocument = new TypedDocumentString(`
    mutation CreateReadingList($input: ReadingListInput!) {
  createReadingList(input: $input) {
    id
    name
  }
}
    `) as unknown as TypedDocumentString<CreateReadingListMutation, CreateReadingListMutationVariables>;
export const UpdateReadingListDocument = new TypedDocumentString(`
    mutation UpdateReadingList($id: ID!, $input: UpdateReadingListInput!) {
  updateReadingList(id: $id, input: $input) {
    id
  }
}
    `) as unknown as TypedDocumentString<UpdateReadingListMutation, UpdateReadingListMutationVariables>;
export const DeleteReadingListDocument = new TypedDocumentString(`
    mutation DeleteReadingList($id: String!) {
  deleteReadingList(id: $id) {
    id
  }
}
    `) as unknown as TypedDocumentString<DeleteReadingListMutation, DeleteReadingListMutationVariables>;
export const AddBooksToReadingListDocument = new TypedDocumentString(`
    mutation AddBooksToReadingList($id: ID!, $mediaIds: [ID!]!) {
  addBooksToReadingList(id: $id, mediaIds: $mediaIds) {
    id
  }
}
    `) as unknown as TypedDocumentString<AddBooksToReadingListMutation, AddBooksToReadingListMutationVariables>;
export const RemoveBooksFromReadingListDocument = new TypedDocumentString(`
    mutation RemoveBooksFromReadingList($id: ID!, $mediaIds: [ID!]!) {
  removeBooksFromReadingList(id: $id, mediaIds: $mediaIds) {
    id
  }
}
    `) as unknown as TypedDocumentString<RemoveBooksFromReadingListMutation, RemoveBooksFromReadingListMutationVariables>;
export const SeriesBookGridDocument = new TypedDocumentString(`
    query SeriesBookGrid($id: String!, $pagination: Pagination) {
  media(filter: {seriesId: {eq: $id}}, pagination: $pagination) {
    nodes {
      id
      thumbnail {
        url
      }
      pages
      extension
    }
    pageInfo {
      __typename
      ... on CursorPaginationInfo {
        currentCursor
        nextCursor
        limit
      }
    }
  }
}
    `) as unknown as TypedDocumentString<SeriesBookGridQuery, SeriesBookGridQueryVariables>;
export const SeriesBooksAlphabetDocument = new TypedDocumentString(`
    query SeriesBooksAlphabet($id: ID!) {
  seriesById(id: $id) {
    mediaAlphabet
  }
}
    `) as unknown as TypedDocumentString<SeriesBooksAlphabetQuery, SeriesBooksAlphabetQueryVariables>;
export const SeriesActionCompleteDocument = new TypedDocumentString(`
    mutation SeriesActionComplete($id: ID!) {
  finishSeriesProgress(id: $id)
}
    `) as unknown as TypedDocumentString<SeriesActionCompleteMutation, SeriesActionCompleteMutationVariables>;
export const SeriesHeaderAnalyzeDocument = new TypedDocumentString(`
    mutation SeriesHeaderAnalyze($id: ID!) {
  analyzeSeries(id: $id)
}
    `) as unknown as TypedDocumentString<SeriesHeaderAnalyzeMutation, SeriesHeaderAnalyzeMutationVariables>;
export const SeriesOverviewSheetExtasDocument = new TypedDocumentString(`
    query SeriesOverviewSheetExtas($id: ID!) {
  seriesById(id: $id) {
    id
    metadata {
      publisher
      year
      summary
      links
    }
    upNext(take: 10) {
      id
      ...SimpleBookCard
    }
  }
}
    fragment SimpleBookCard on Media {
  id
  resolvedName
  createdAt
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
  }
}`) as unknown as TypedDocumentString<SeriesOverviewSheetExtasQuery, SeriesOverviewSheetExtasQueryVariables>;
export const SeriesTagEditorSetTagsDocument = new TypedDocumentString(`
    mutation SeriesTagEditorSetTags($id: ID!, $tags: [String!]!) {
  setSeriesTags(id: $id, tags: $tags) {
    id
    tags {
      id
      name
    }
  }
}
    `) as unknown as TypedDocumentString<SeriesTagEditorSetTagsMutation, SeriesTagEditorSetTagsMutationVariables>;
export const UpdateSeriesMetadataDocument = new TypedDocumentString(`
    mutation UpdateSeriesMetadata($id: ID!, $input: SeriesMetadataInput!) {
  updateSeriesMetadata(id: $id, input: $input) {
    id
  }
}
    `) as unknown as TypedDocumentString<UpdateSeriesMetadataMutation, UpdateSeriesMetadataMutationVariables>;
export const SeriesEditorSetLockedFieldsDocument = new TypedDocumentString(`
    mutation SeriesEditorSetLockedFields($seriesId: ID!, $lockedFields: [MetadataField!]!) {
  setSeriesLockedFields(seriesId: $seriesId, lockedFields: $lockedFields) {
    id
  }
}
    `) as unknown as TypedDocumentString<SeriesEditorSetLockedFieldsMutation, SeriesEditorSetLockedFieldsMutationVariables>;
export const SeriesLayoutDocument = new TypedDocumentString(`
    query SeriesLayout($id: ID!) {
  seriesById(id: $id) {
    kind
    id
    path
    library {
      id
      name
    }
    resolvedName
    resolvedDescription
    stats {
      bookCount
      completedBooks
      inProgressBooks
      totalBytes
      totalReadingTimeSeconds
    }
    tags {
      id
      name
    }
    thumbnail {
      url
      metadata {
        averageColor
        thumbhash
        colors {
          color
          percentage
        }
      }
    }
    createdAt
    updatedAt
  }
}
    `) as unknown as TypedDocumentString<SeriesLayoutQuery, SeriesLayoutQueryVariables>;
export const SeriesBooksSceneDocument = new TypedDocumentString(`
    query SeriesBooksScene($filter: MediaFilterInput!, $orderBy: [MediaOrderBy!]!, $pagination: Pagination!) {
  media(filter: $filter, orderBy: $orderBy, pagination: $pagination) {
    nodes {
      id
      ...BookCard
      ...BookMetadata
    }
    pageInfo {
      __typename
      ... on OffsetPaginationInfo {
        currentPage
        totalPages
        pageSize
        pageOffset
        zeroBased
      }
    }
  }
}
    fragment BookCard on Media {
  id
  resolvedName
  extension
  pages
  size
  status
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
  readProgress {
    percentageCompleted
    page
    updatedAt
    locator {
      href
    }
  }
  readHistory {
    __typename
    completedAt
  }
  createdAt
  libraryConfig {
    skipBookOverview
  }
  metadata {
    number
    rating
    ageRating
    language
  }
}
fragment BookMetadata on Media {
  metadata {
    ageRating
    rating
    characters
    colorists
    coverArtists
    editors
    genres
    inkers
    letterers
    links
    pencillers
    publisher
    series
    teams
    writers
    year
    month
    day
    volume
    number
  }
}`) as unknown as TypedDocumentString<SeriesBooksSceneQuery, SeriesBooksSceneQueryVariables>;
export const SeriesThumbnailSelectorUpdateDocument = new TypedDocumentString(`
    mutation SeriesThumbnailSelectorUpdate($id: ID!, $input: UpdateThumbnailInput!) {
  updateSeriesThumbnail(id: $id, input: $input) {
    id
    thumbnail {
      url
    }
  }
}
    `) as unknown as TypedDocumentString<SeriesThumbnailSelectorUpdateMutation, SeriesThumbnailSelectorUpdateMutationVariables>;
export const SeriesThumbnailSelectorUploadDocument = new TypedDocumentString(`
    mutation SeriesThumbnailSelectorUpload($id: ID!, $file: Upload!) {
  uploadSeriesThumbnail(id: $id, file: $file) {
    id
    thumbnail {
      url
    }
  }
}
    `) as unknown as TypedDocumentString<SeriesThumbnailSelectorUploadMutation, SeriesThumbnailSelectorUploadMutationVariables>;
export const UseCoreEventDocument = new TypedDocumentString(`
    subscription UseCoreEvent {
  readEvents {
    __typename
    ... on CreatedManySeries {
      count
      libraryId
    }
    ... on DiscoveredMissingLibrary {
      id
    }
    ... on JobStarted {
      id
    }
    ... on JobUpdate {
      __typename
      id
      status
      message
      completedTasks
      remainingTasks
      completedSubtasks
      totalSubtasks
      subtitle
    }
    ... on JobOutput {
      id
      output {
        __typename
        ... on LibraryScanOutput {
          createdMedia
          createdSeries
          updatedMedia
          updatedSeries
        }
        ... on SeriesScanOutput {
          createdMedia
          updatedMedia
        }
      }
    }
  }
}
    `) as unknown as TypedDocumentString<UseCoreEventSubscription, UseCoreEventSubscriptionVariables>;
export const UsePreferencesDocument = new TypedDocumentString(`
    mutation UsePreferences($input: UpdateUserPreferencesInput!) {
  updateViewerPreferences(input: $input) {
    __typename
  }
}
    `) as unknown as TypedDocumentString<UsePreferencesMutation, UsePreferencesMutationVariables>;
export const BooksAlphabetDocument = new TypedDocumentString(`
    query BooksAlphabet {
  mediaAlphabet
}
    `) as unknown as TypedDocumentString<BooksAlphabetQuery, BooksAlphabetQueryVariables>;
export const BooksInSeriesDocument = new TypedDocumentString(`
    query BooksInSeries($seriesId: ID!, $filter: MediaFilterInput!, $orderBy: [MediaOrderBy!]!, $pagination: Pagination!) {
  seriesById(id: $seriesId) {
    id
    resolvedName
  }
  media(filter: $filter, orderBy: $orderBy, pagination: $pagination) {
    nodes {
      id
      ...BookCard
    }
    pageInfo {
      __typename
      ... on OffsetPaginationInfo {
        currentPage
        totalPages
        totalItems
        pageSize
        pageOffset
        zeroBased
      }
    }
  }
}
    fragment BookCard on Media {
  id
  resolvedName
  extension
  pages
  size
  status
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
  readProgress {
    percentageCompleted
    page
    updatedAt
    locator {
      href
    }
  }
  readHistory {
    __typename
    completedAt
  }
  createdAt
  libraryConfig {
    skipBookOverview
  }
  metadata {
    number
    rating
    ageRating
    language
  }
}`) as unknown as TypedDocumentString<BooksInSeriesQuery, BooksInSeriesQueryVariables>;
export const MoreByAuthorQueryDocument = new TypedDocumentString(`
    query MoreByAuthorQuery($filter: MediaFilterInput!, $pagination: Pagination) {
  media(filter: $filter, pagination: $pagination) {
    nodes {
      id
      seriesId
      ...BookCard
    }
  }
}
    fragment BookCard on Media {
  id
  resolvedName
  extension
  pages
  size
  status
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
  readProgress {
    percentageCompleted
    page
    updatedAt
    locator {
      href
    }
  }
  readHistory {
    __typename
    completedAt
  }
  createdAt
  libraryConfig {
    skipBookOverview
  }
  metadata {
    number
    rating
    ageRating
    language
  }
}`) as unknown as TypedDocumentString<MoreByAuthorQueryQuery, MoreByAuthorQueryQueryVariables>;
export const BookReaderSceneDocument = new TypedDocumentString(`
    query BookReaderScene($id: ID!) {
  mediaById(id: $id) {
    id
    resolvedName
    pages
    extension
    readProgress {
      percentageCompleted
      page
      elapsedSeconds
    }
    libraryConfig {
      defaultReadingImageScaleFit
      defaultReadingMode
      defaultReadingDir
    }
    metadata {
      genres
    }
    analysisData {
      dimensions {
        height
        width
      }
    }
    nextInSeries(pagination: {cursor: {limit: 1}}) {
      nodes {
        id
        name: resolvedName
        thumbnail {
          url
        }
      }
    }
  }
}
    `) as unknown as TypedDocumentString<BookReaderSceneQuery, BookReaderSceneQueryVariables>;
export const UpdateReadProgressDocument = new TypedDocumentString(`
    mutation UpdateReadProgress($id: ID!, $input: MediaProgressInput!) {
  updateMediaProgress(id: $id, input: $input) {
    __typename
  }
}
    `) as unknown as TypedDocumentString<UpdateReadProgressMutation, UpdateReadProgressMutationVariables>;
export const BookMetadataSearchProvidersDocument = new TypedDocumentString(`
    query BookMetadataSearchProviders {
  metadataProviderConfigs {
    id
    providerType
    enabled
  }
}
    `) as unknown as TypedDocumentString<BookMetadataSearchProvidersQuery, BookMetadataSearchProvidersQueryVariables>;
export const BookMetadataSearchContextDocument = new TypedDocumentString(`
    query BookMetadataSearchContext($id: ID!) {
  mediaById(id: $id) {
    id
    series {
      id
      metadata {
        comicid
      }
    }
  }
}
    `) as unknown as TypedDocumentString<BookMetadataSearchContextQuery, BookMetadataSearchContextQueryVariables>;
export const SearchMediaMetadataDocument = new TypedDocumentString(`
    mutation SearchMediaMetadata($id: ID!, $search: MediaMetadataSearchInput) {
  fetchMediaMetadata(id: $id, search: $search) {
    ...PendingMatchRecord
  }
}
    fragment PendingMatchRecord on MetadataFetchRecord {
  id
  status
  mediaId
  seriesId
  rawHits
  matchCandidates {
    provider
    externalId
    metadata {
      __typename
      ... on ExternalMediaMetadata {
        title
        seriesName
        seriesExternalId
        summary
        pageCount
        number
        day
        month
        year
        genres
        tags
        isbn
        isbn13
        writers
        artists
        colorists
        letterers
        coverArtists
      }
      ... on ExternalSeriesMetadata {
        seriesTitle: title
        alternativeTitles
        summary
        volumeCount
        coverUrl
        status
        year
        endYear
        genres
        tags
        authors
        ageRating
        publisher
      }
    }
    confidence
    confidenceFactors {
      factor
      weight
      matched
    }
  }
  addedAt
  updatedAt
  media {
    id
    resolvedName
    metadata {
      title
      summary
      series
      number
      genres
      writers
      colorists
      letterers
      coverArtists
      publisher
      year
      month
      day
      pageCount
      identifierIsbn
      lockedFields
    }
  }
  series {
    id
    resolvedName
    metadata {
      title
      summary
      genres
      writers
      publisher
      year
      status
      ageRating
      volume
      lockedFields
    }
  }
}`) as unknown as TypedDocumentString<SearchMediaMetadataMutation, SearchMediaMetadataMutationVariables>;
export const BookSettingsSceneDocument = new TypedDocumentString(`
    query BookSettingsScene($id: ID!) {
  mediaById(id: $id) {
    id
    resolvedName
    ...BookHeader
    tags {
      id
      name
    }
    metadata {
      ...MediaMetadataEditor
    }
  }
}
    fragment BookHeader on Media {
  id
  resolvedName
  pages
  extension
  series {
    id
    resolvedName
  }
  library {
    id
    name
  }
  ...BookReadingActions
}
fragment MediaMetadataEditor on MediaMetadata {
  ageRating
  characters
  colorists
  coverArtists
  day
  editors
  format
  identifierAmazon
  identifierCalibre
  identifierGoogle
  identifierIsbn
  identifierMobiAsin
  identifierUuid
  genres
  inkers
  language
  letterers
  links
  month
  notes
  number
  pageCount
  pencillers
  publisher
  series
  seriesGroup
  storyArc
  storyArcNumber
  summary
  teams
  title
  titleSort
  volume
  writers
  year
  lockedFields
}
fragment BookReadingActions on Media {
  id
  extension
  readProgress {
    page
    locator {
      href
    }
  }
  readHistory {
    __typename
  }
}`) as unknown as TypedDocumentString<BookSettingsSceneQuery, BookSettingsSceneQueryVariables>;
export const BookTagEditorSetTagsDocument = new TypedDocumentString(`
    mutation BookTagEditorSetTags($id: ID!, $tags: [String!]!) {
  setMediaTags(id: $id, tags: $tags) {
    id
    tags {
      id
      name
    }
  }
}
    `) as unknown as TypedDocumentString<BookTagEditorSetTagsMutation, BookTagEditorSetTagsMutationVariables>;
export const BookClubLayoutDocument = new TypedDocumentString(`
    query BookClubLayout($slug: String!) {
  bookClubBySlug(slug: $slug) {
    id
    name
    slug
    description
    isPrivate
    roleSpec
    creator {
      id
      displayName
      avatarUrl
    }
    membersCount
    membership {
      role
      avatarUrl
      isCreator
    }
    currentBook {
      id
      title
      author
      imageUrl
      entity {
        id
        thumbnail {
          url
        }
      }
      ...BookClubBookItem
    }
    createdAt
  }
}
    fragment BookClubBookItem on BookClubBook {
  id
  title
  author
  imageUrl
  url
  entity {
    __typename
    id
    resolvedName
    metadata {
      writers
    }
    thumbnail {
      url
    }
  }
  completedAt
  addedAt
}`) as unknown as TypedDocumentString<BookClubLayoutQuery, BookClubLayoutQueryVariables>;
export const UpdateBookClubDocument = new TypedDocumentString(`
    mutation UpdateBookClub($id: ID!, $input: UpdateBookClubInput!) {
  updateBookClub(id: $id, input: $input) {
    id
    name
    emoji
    isPrivate
    roleSpec
    description
  }
}
    `) as unknown as TypedDocumentString<UpdateBookClubMutation, UpdateBookClubMutationVariables>;
export const UserBookClubsSceneDocument = new TypedDocumentString(`
    query UserBookClubsScene {
  bookClubs(all: false) {
    id
    name
    slug
    description
    membersCount
    currentBook {
      id
    }
  }
}
    `) as unknown as TypedDocumentString<UserBookClubsSceneQuery, UserBookClubsSceneQueryVariables>;
export const CreateBookClubFormDocument = new TypedDocumentString(`
    query CreateBookClubForm {
  bookClubs {
    name
    slug
  }
}
    `) as unknown as TypedDocumentString<CreateBookClubFormQuery, CreateBookClubFormQueryVariables>;
export const CreateBookClubSceneDocument = new TypedDocumentString(`
    mutation CreateBookClubScene($input: CreateBookClubInput!) {
  createBookClub(input: $input) {
    id
    slug
  }
}
    `) as unknown as TypedDocumentString<CreateBookClubSceneMutation, CreateBookClubSceneMutationVariables>;
export const BookClubBasicSettingsSceneDocument = new TypedDocumentString(`
    query BookClubBasicSettingsScene {
  bookClubs(all: true) {
    id
    name
    slug
  }
}
    `) as unknown as TypedDocumentString<BookClubBasicSettingsSceneQuery, BookClubBasicSettingsSceneQueryVariables>;
export const BookClubMembersTableDocument = new TypedDocumentString(`
    query BookClubMembersTable($id: ID!) {
  bookClubById(id: $id) {
    id
    members {
      id
      avatarUrl
      isCreator
      displayName
      role
      userId
    }
  }
}
    `) as unknown as TypedDocumentString<BookClubMembersTableQuery, BookClubMembersTableQueryVariables>;
export const RemoveBookClubMemberDocument = new TypedDocumentString(`
    mutation RemoveBookClubMember($bookClubId: ID!, $memberId: ID!) {
  removeBookClubMember(bookClubId: $bookClubId, memberId: $memberId) {
    id
  }
}
    `) as unknown as TypedDocumentString<RemoveBookClubMemberMutation, RemoveBookClubMemberMutationVariables>;
export const BookSearchSceneDocument = new TypedDocumentString(`
    query BookSearchScene($filter: MediaFilterInput!, $orderBy: [MediaOrderBy!]!, $pagination: Pagination!) {
  media(filter: $filter, orderBy: $orderBy, pagination: $pagination) {
    nodes {
      id
      ...BookCard
      ...BookMetadata
    }
    pageInfo {
      __typename
      ... on OffsetPaginationInfo {
        currentPage
        totalPages
        pageSize
        pageOffset
        zeroBased
      }
    }
  }
}
    fragment BookCard on Media {
  id
  resolvedName
  extension
  pages
  size
  status
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
  readProgress {
    percentageCompleted
    page
    updatedAt
    locator {
      href
    }
  }
  readHistory {
    __typename
    completedAt
  }
  createdAt
  libraryConfig {
    skipBookOverview
  }
  metadata {
    number
    rating
    ageRating
    language
  }
}
fragment BookMetadata on Media {
  metadata {
    ageRating
    rating
    characters
    colorists
    coverArtists
    editors
    genres
    inkers
    letterers
    links
    pencillers
    publisher
    series
    teams
    writers
    year
    month
    day
    volume
    number
  }
}`) as unknown as TypedDocumentString<BookSearchSceneQuery, BookSearchSceneQueryVariables>;
export const BrowseIndexSceneDocument = new TypedDocumentString(`
    query BrowseIndexScene($field: MetadataFacetField!) {
  metadataValueCounts(field: $field) {
    value
    count
  }
}
    `) as unknown as TypedDocumentString<BrowseIndexSceneQuery, BrowseIndexSceneQueryVariables>;
export const CreateLibrarySceneExistingLibrariesDocument = new TypedDocumentString(`
    query CreateLibrarySceneExistingLibraries {
  libraries(pagination: {none: {unpaginated: true}}) {
    nodes {
      id
      name
      path
    }
  }
}
    `) as unknown as TypedDocumentString<CreateLibrarySceneExistingLibrariesQuery, CreateLibrarySceneExistingLibrariesQueryVariables>;
export const CreateLibrarySceneCreateLibraryDocument = new TypedDocumentString(`
    mutation CreateLibrarySceneCreateLibrary($input: CreateOrUpdateLibraryInput!) {
  createLibrary(input: $input) {
    id
  }
}
    `) as unknown as TypedDocumentString<CreateLibrarySceneCreateLibraryMutation, CreateLibrarySceneCreateLibraryMutationVariables>;
export const CreateSmartListFormDocument = new TypedDocumentString(`
    query CreateSmartListForm {
  smartLists(input: {mine: true}) {
    name
  }
}
    `) as unknown as TypedDocumentString<CreateSmartListFormQuery, CreateSmartListFormQueryVariables>;
export const CreateSmartListSceneDocument = new TypedDocumentString(`
    mutation CreateSmartListScene($input: SaveSmartListInput!) {
  createSmartList(input: $input) {
    id
    name
  }
}
    `) as unknown as TypedDocumentString<CreateSmartListSceneMutation, CreateSmartListSceneMutationVariables>;
export const ContinueReadingMediaDocument = new TypedDocumentString(`
    query ContinueReadingMedia($pagination: Pagination!) {
  keepReading(pagination: $pagination) {
    nodes {
      id
      ...ContinueReadingBook
    }
    pageInfo {
      __typename
      ... on CursorPaginationInfo {
        currentCursor
        nextCursor
        limit
      }
      ... on OffsetPaginationInfo {
        currentPage
        totalPages
        pageSize
        pageOffset
        zeroBased
      }
    }
  }
}
    fragment ContinueReadingBook on Media {
  id
  resolvedName
  pages
  extension
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
  }
  readProgress {
    percentageCompleted
    page
    updatedAt
    locator {
      href
    }
  }
}`) as unknown as TypedDocumentString<ContinueReadingMediaQuery, ContinueReadingMediaQueryVariables>;
export const HomeSceneQueryDocument = new TypedDocumentString(`
    query HomeSceneQuery {
  numberOfLibraries
}
    `) as unknown as TypedDocumentString<HomeSceneQueryQuery, HomeSceneQueryQueryVariables>;
export const HomeSearchDocument = new TypedDocumentString(`
    query HomeSearch($filter: MediaFilterInput!) {
  media(filter: $filter, pagination: {offset: {page: 1, pageSize: 6}}) {
    nodes {
      id
      resolvedName
      thumbnail {
        url
      }
      metadata {
        writers
        rating
      }
    }
    pageInfo {
      __typename
      ... on OffsetPaginationInfo {
        totalPages
      }
    }
  }
}
    `) as unknown as TypedDocumentString<HomeSearchQuery, HomeSearchQueryVariables>;
export const OnDeckBooksWebDocument = new TypedDocumentString(`
    query OnDeckBooksWeb($pagination: Pagination!) {
  onDeck(pagination: $pagination) {
    nodes {
      id
      ...OnDeckBook
    }
    pageInfo {
      __typename
      ... on OffsetPaginationInfo {
        currentPage
        totalPages
        pageSize
        pageOffset
        zeroBased
      }
    }
  }
}
    fragment OnDeckBook on Media {
  id
  metadata {
    number
  }
  resolvedName
  seriesPosition
  series {
    mediaCount
    metadata {
      totalIssues
    }
  }
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
  }
}`) as unknown as TypedDocumentString<OnDeckBooksWebQuery, OnDeckBooksWebQueryVariables>;
export const RecentlyAddedMediaDocument = new TypedDocumentString(`
    query RecentlyAddedMedia($pagination: Pagination!) {
  recentlyAddedMedia(pagination: $pagination) {
    nodes {
      id
      ...RecentlyAddedBook
    }
    pageInfo {
      __typename
      ... on CursorPaginationInfo {
        currentCursor
        nextCursor
        limit
      }
    }
  }
}
    fragment RecentlyAddedBook on Media {
  id
  resolvedName
  createdAt
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
  }
}`) as unknown as TypedDocumentString<RecentlyAddedMediaQuery, RecentlyAddedMediaQueryVariables>;
export const RecentlyAddedSeriesDocument = new TypedDocumentString(`
    query RecentlyAddedSeries($pagination: Pagination!) {
  recentlyAddedSeries(pagination: $pagination) {
    nodes {
      id
      resolvedName
      mediaCount
      percentageCompleted
      status
      createdAt
      oneshotBook {
        id
      }
      stackedThumbnails {
        url
        metadata {
          averageColor
          colors {
            color
            percentage
          }
          thumbhash
        }
      }
    }
    pageInfo {
      __typename
      ... on CursorPaginationInfo {
        currentCursor
        nextCursor
        limit
      }
    }
  }
}
    `) as unknown as TypedDocumentString<RecentlyAddedSeriesQuery, RecentlyAddedSeriesQueryVariables>;
export const HomeArrangementPreferencesDocument = new TypedDocumentString(`
    query HomeArrangementPreferences {
  me {
    preferences {
      homeArrangement {
        sections {
          visible
          config {
            __typename
            ... on InProgressBooks {
              name
            }
            ... on OnDeckBooks {
              name
            }
            ... on RecentlyAdded {
              entity
              name
            }
          }
        }
      }
    }
  }
}
    `) as unknown as TypedDocumentString<HomeArrangementPreferencesQuery, HomeArrangementPreferencesQueryVariables>;
export const UpdateHomeArrangementDocument = new TypedDocumentString(`
    mutation UpdateHomeArrangement($input: HomeArrangementInput!) {
  updateHomeArrangement(input: $input) {
    sections {
      visible
      config {
        __typename
        ... on InProgressBooks {
          name
        }
        ... on OnDeckBooks {
          name
        }
        ... on RecentlyAdded {
          entity
          name
        }
      }
    }
  }
}
    `) as unknown as TypedDocumentString<UpdateHomeArrangementMutation, UpdateHomeArrangementMutationVariables>;
export const IntakeItemsDocument = new TypedDocumentString(`
    query IntakeItems {
  intakeItems {
    id
    libraryId
    libraryName
    state
    kind
    name
    error
    createdAt
    filedAt
    problems
    mediaIds
    files {
      originalName
      size
      format
      volume
      title
    }
    proposal {
      metadata {
        title
        authors
        series
        seriesIndex
        language
        year
        publisher
        description
        isbn
        subjects
        genres
        sources {
          field
          source
        }
      }
      seriesKind
      authorFolders {
        folder
        books
        confidence
        reason
        isNew
      }
      seriesFolders {
        folder
        books
        confidence
        reason
        isNew
      }
      genreSuggestions {
        value
        confidence
        reasons
        libraryCount
        selected
        kind
      }
      similarBooks {
        mediaId
        title
        authors
        genres
        reason
      }
      duplicates {
        mediaId
        path
        title
        reason
        fileName
      }
      warnings
      confidence
      lookedUpOnline
    }
    decision {
      metadata {
        title
        authors
        series
        seriesIndex
        language
        year
        publisher
        genres
        ageRating
        sources {
          field
          source
        }
      }
      seriesKind
      folder
      fileNames
      volumes
    }
    destination {
      relativeDir
      dirExists
      files {
        originalName
        relativePath
        exists
      }
    }
    result {
      paths
      notes
    }
  }
}
    `) as unknown as TypedDocumentString<IntakeItemsQuery, IntakeItemsQueryVariables>;
export const IntakeLibrariesDocument = new TypedDocumentString(`
    query IntakeLibraries {
  intakeLibraries {
    id
    name
  }
}
    `) as unknown as TypedDocumentString<IntakeLibrariesQuery, IntakeLibrariesQueryVariables>;
export const IntakeVocabularyDocument = new TypedDocumentString(`
    query IntakeVocabulary($libraryId: ID!, $search: String, $limit: Int! = 20) {
  intakeVocabulary(libraryId: $libraryId, search: $search, limit: $limit) {
    value
    count
    kind
  }
}
    `) as unknown as TypedDocumentString<IntakeVocabularyQuery, IntakeVocabularyQueryVariables>;
export const IntakeFoldersDocument = new TypedDocumentString(`
    query IntakeFolders($libraryId: ID!, $seriesKind: IntakeSeriesKind, $search: String, $limit: Int! = 20) {
  intakeFolders(
    libraryId: $libraryId
    seriesKind: $seriesKind
    search: $search
    limit: $limit
  ) {
    name
    books
  }
}
    `) as unknown as TypedDocumentString<IntakeFoldersQuery, IntakeFoldersQueryVariables>;
export const IntakeUploadDocument = new TypedDocumentString(`
    mutation IntakeUpload($libraryId: ID!, $files: [Upload!]!, $relativePaths: [String!], $mode: IntakeUploadMode! = AUTO) {
  intakeUpload(
    libraryId: $libraryId
    files: $files
    relativePaths: $relativePaths
    mode: $mode
  ) {
    id
    state
  }
}
    `) as unknown as TypedDocumentString<IntakeUploadMutation, IntakeUploadMutationVariables>;
export const IntakeReviewDocument = new TypedDocumentString(`
    mutation IntakeReview($id: ID!, $input: IntakeReviewInput!) {
  intakeReview(id: $id, input: $input) {
    id
    state
  }
}
    `) as unknown as TypedDocumentString<IntakeReviewMutation, IntakeReviewMutationVariables>;
export const IntakeReanalyseDocument = new TypedDocumentString(`
    mutation IntakeReanalyse($id: ID!, $online: Boolean) {
  intakeReanalyse(id: $id, online: $online) {
    id
    state
  }
}
    `) as unknown as TypedDocumentString<IntakeReanalyseMutation, IntakeReanalyseMutationVariables>;
export const IntakeAcceptDocument = new TypedDocumentString(`
    mutation IntakeAccept($ids: [ID!]!) {
  intakeAccept(ids: $ids) {
    id
    state
    error
  }
}
    `) as unknown as TypedDocumentString<IntakeAcceptMutation, IntakeAcceptMutationVariables>;
export const IntakeDiscardDocument = new TypedDocumentString(`
    mutation IntakeDiscard($ids: [ID!]!) {
  intakeDiscard(ids: $ids)
}
    `) as unknown as TypedDocumentString<IntakeDiscardMutation, IntakeDiscardMutationVariables>;
export const LibrariesSceneDocument = new TypedDocumentString(`
    query LibrariesScene {
  libraries(pagination: {none: {unpaginated: true}}) {
    nodes {
      id
      ...LibraryWebGridItem
    }
  }
  librariesStats {
    seriesCount
    bookCount
  }
}
    fragment LibraryWebGridItem on Library {
  id
  name
  series(take: 5) {
    thumbnail {
      url
      metadata {
        averageColor
        colors {
          color
          percentage
        }
        thumbhash
      }
    }
  }
  stats {
    seriesCount
    bookCount
  }
}`) as unknown as TypedDocumentString<LibrariesSceneQuery, LibrariesSceneQueryVariables>;
export const LibrariesSceneScanAllLibrariesDocument = new TypedDocumentString(`
    mutation LibrariesSceneScanAllLibraries {
  scanAllLibraries
}
    `) as unknown as TypedDocumentString<LibrariesSceneScanAllLibrariesMutation, LibrariesSceneScanAllLibrariesMutationVariables>;
export const LibraryLayoutDocument = new TypedDocumentString(`
    query LibraryLayout($id: ID!) {
  libraryById(id: $id) {
    id
    name
    description
    path
    stats {
      seriesCount
      bookCount
      completedBooks
      inProgressBooks
      totalBytes
      totalReadingTimeSeconds
    }
    tags {
      id
      name
    }
    thumbnail {
      url
      metadata {
        averageColor
        thumbhash
        colors {
          color
          percentage
        }
      }
    }
    config {
      organizationMode
      defaultLibraryViewMode
      hideSeriesView
    }
    ...LibrarySettingsConfig
    config {
      oneshotsDirectory
    }
  }
}
    fragment LibrarySettingsConfig on Library {
  config {
    id
    convertRarToZip
    hardDeleteConversions
    defaultReadingDir
    defaultReadingMode
    defaultReadingImageScaleFit
    defaultLibraryViewMode
    hideSeriesView
    skipBookOverview
    generateFileHashes
    generateKoreaderHashes
    processMetadata
    watch
    libraryPattern
    organizationMode
    libraryType
    thumbnailConfig {
      __typename
      resizeMethod {
        __typename
        ... on ScaleEvenlyByFactor {
          factor
        }
        ... on ExactDimensionResize {
          width
          height
        }
        ... on ScaledDimensionResize {
          dimension
          size
        }
      }
      format
      quality
      page
    }
    processThumbnailColorsEvenWithoutConfig
    ignoreRules
    oneshotsDirectory
  }
}`) as unknown as TypedDocumentString<LibraryLayoutQuery, LibraryLayoutQueryVariables>;
export const VisitLibraryDocument = new TypedDocumentString(`
    mutation VisitLibrary($id: ID!) {
  visitLibrary(id: $id) {
    id
  }
}
    `) as unknown as TypedDocumentString<VisitLibraryMutation, VisitLibraryMutationVariables>;
export const LibraryBooksSceneDocument = new TypedDocumentString(`
    query LibraryBooksScene($filter: MediaFilterInput!, $orderBy: [MediaOrderBy!]!, $pagination: Pagination!) {
  media(filter: $filter, orderBy: $orderBy, pagination: $pagination) {
    nodes {
      id
      ...BookCard
      ...BookMetadata
    }
    pageInfo {
      __typename
      ... on OffsetPaginationInfo {
        currentPage
        totalPages
        pageSize
        pageOffset
        zeroBased
      }
    }
  }
}
    fragment BookCard on Media {
  id
  resolvedName
  extension
  pages
  size
  status
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
  readProgress {
    percentageCompleted
    page
    updatedAt
    locator {
      href
    }
  }
  readHistory {
    __typename
    completedAt
  }
  createdAt
  libraryConfig {
    skipBookOverview
  }
  metadata {
    number
    rating
    ageRating
    language
  }
}
fragment BookMetadata on Media {
  metadata {
    ageRating
    rating
    characters
    colorists
    coverArtists
    editors
    genres
    inkers
    letterers
    links
    pencillers
    publisher
    series
    teams
    writers
    year
    month
    day
    volume
    number
  }
}`) as unknown as TypedDocumentString<LibraryBooksSceneQuery, LibraryBooksSceneQueryVariables>;
export const LibrarySeriesDocument = new TypedDocumentString(`
    query LibrarySeries($filter: SeriesFilterInput!, $orderBy: [SeriesOrderBy!]!, $pagination: Pagination!) {
  series(filter: $filter, orderBy: $orderBy, pagination: $pagination) {
    nodes {
      id
      resolvedName
      mediaCount
      percentageCompleted
      status
      stackedThumbnails {
        url
        metadata {
          averageColor
          colors {
            color
            percentage
          }
          thumbhash
        }
      }
      oneshotBook {
        id
      }
    }
    pageInfo {
      __typename
      ... on OffsetPaginationInfo {
        totalPages
        currentPage
        pageSize
        pageOffset
        zeroBased
      }
    }
  }
}
    `) as unknown as TypedDocumentString<LibrarySeriesQuery, LibrarySeriesQueryVariables>;
export const LibrarySeriesGridDocument = new TypedDocumentString(`
    query LibrarySeriesGrid($id: String!, $pagination: Pagination) {
  series(filter: {libraryId: {eq: $id}}, pagination: $pagination) {
    nodes {
      id
      thumbnail {
        url
      }
    }
    pageInfo {
      __typename
      ... on CursorPaginationInfo {
        currentCursor
        nextCursor
        limit
      }
    }
  }
}
    `) as unknown as TypedDocumentString<LibrarySeriesGridQuery, LibrarySeriesGridQueryVariables>;
export const LibrarySettingsRouterEditLibraryMutationDocument = new TypedDocumentString(`
    mutation LibrarySettingsRouterEditLibraryMutation($id: ID!, $input: CreateOrUpdateLibraryInput!) {
  updateLibrary(id: $id, input: $input) {
    id
  }
}
    `) as unknown as TypedDocumentString<LibrarySettingsRouterEditLibraryMutationMutation, LibrarySettingsRouterEditLibraryMutationMutationVariables>;
export const LibrarySettingsRouterScanLibraryMutationDocument = new TypedDocumentString(`
    mutation LibrarySettingsRouterScanLibraryMutation($id: ID!, $options: JSON) {
  scanLibrary(id: $id, options: $options)
}
    `) as unknown as TypedDocumentString<LibrarySettingsRouterScanLibraryMutationMutation, LibrarySettingsRouterScanLibraryMutationMutationVariables>;
export const BasicSettingsSceneExistingLibrariesDocument = new TypedDocumentString(`
    query BasicSettingsSceneExistingLibraries {
  libraries(pagination: {none: {unpaginated: true}}) {
    nodes {
      id
      name
      path
    }
  }
}
    `) as unknown as TypedDocumentString<BasicSettingsSceneExistingLibrariesQuery, BasicSettingsSceneExistingLibrariesQueryVariables>;
export const LibraryExclusionsUsersQueryDocument = new TypedDocumentString(`
    query LibraryExclusionsUsersQuery {
  users(pagination: {none: {unpaginated: true}}) {
    nodes {
      id
      username
    }
  }
}
    `) as unknown as TypedDocumentString<LibraryExclusionsUsersQueryQuery, LibraryExclusionsUsersQueryQueryVariables>;
export const LibraryExclusionsQueryDocument = new TypedDocumentString(`
    query LibraryExclusionsQuery($id: ID!) {
  libraryById(id: $id) {
    excludedUsers {
      id
      username
    }
  }
}
    `) as unknown as TypedDocumentString<LibraryExclusionsQueryQuery, LibraryExclusionsQueryQueryVariables>;
export const UpdateLibraryExclusionsDocument = new TypedDocumentString(`
    mutation UpdateLibraryExclusions($id: ID!, $userIds: [String!]!) {
  updateLibraryExcludedUsers(id: $id, userIds: $userIds) {
    id
    excludedUsers {
      id
      username
    }
  }
}
    `) as unknown as TypedDocumentString<UpdateLibraryExclusionsMutation, UpdateLibraryExclusionsMutationVariables>;
export const CleanLibraryDocument = new TypedDocumentString(`
    mutation CleanLibrary($id: ID!) {
  cleanLibrary(id: $id) {
    deletedMediaCount
    deletedSeriesCount
    isEmpty
  }
}
    `) as unknown as TypedDocumentString<CleanLibraryMutation, CleanLibraryMutationVariables>;
export const LibraryDeletedBooksDocument = new TypedDocumentString(`
    query LibraryDeletedBooks($libraryId: ID!) {
  deletedBooks(libraryId: $libraryId) {
    book {
      id
    }
  }
  deletedBooksSettings {
    retentionDays
  }
}
    `) as unknown as TypedDocumentString<LibraryDeletedBooksQuery, LibraryDeletedBooksQueryVariables>;
export const LibraryMissingEntitiesDocument = new TypedDocumentString(`
    query LibraryMissingEntities($libraryId: ID!, $pagination: Pagination!) {
  libraryMissingEntities(libraryId: $libraryId, pagination: $pagination) {
    nodes {
      id
      name
      path
      type
    }
    pageInfo {
      __typename
      ... on OffsetPaginationInfo {
        totalPages
        currentPage
        pageSize
        pageOffset
        pageOffset
        zeroBased
        totalItems
      }
    }
  }
}
    `) as unknown as TypedDocumentString<LibraryMissingEntitiesQuery, LibraryMissingEntitiesQueryVariables>;
export const AnalyzeLibraryMediaDocument = new TypedDocumentString(`
    mutation AnalyzeLibraryMedia($id: ID!) {
  analyzeLibrary(id: $id)
}
    `) as unknown as TypedDocumentString<AnalyzeLibraryMediaMutation, AnalyzeLibraryMediaMutationVariables>;
export const InitFetchJobCheckProvidersDocument = new TypedDocumentString(`
    query InitFetchJobCheckProviders {
  metadataProviderConfigs {
    id
  }
}
    `) as unknown as TypedDocumentString<InitFetchJobCheckProvidersQuery, InitFetchJobCheckProvidersQueryVariables>;
export const InitFetchJobDocument = new TypedDocumentString(`
    mutation InitFetchJob($id: ID!) {
  fetchLibraryMetadata(id: $id)
}
    `) as unknown as TypedDocumentString<InitFetchJobMutation, InitFetchJobMutationVariables>;
export const LibraryAddBooksSettingsDocument = new TypedDocumentString(`
    query LibraryAddBooksSettings($libraryId: ID!) {
  libraryIntakeSettings(libraryId: $libraryId) {
    libraryId
    mangaDir
    lightNovelDir
    onlineLookup
  }
}
    `) as unknown as TypedDocumentString<LibraryAddBooksSettingsQuery, LibraryAddBooksSettingsQueryVariables>;
export const UpdateLibraryAddBooksSettingsDocument = new TypedDocumentString(`
    mutation UpdateLibraryAddBooksSettings($libraryId: ID!, $input: LibraryIntakeSettingsInput!) {
  updateLibraryIntakeSettings(libraryId: $libraryId, input: $input) {
    libraryId
  }
}
    `) as unknown as TypedDocumentString<UpdateLibraryAddBooksSettingsMutation, UpdateLibraryAddBooksSettingsMutationVariables>;
export const LibraryPostProcessingDocument = new TypedDocumentString(`
    query LibraryPostProcessing($libraryId: ID!) {
  libraryPostProcessing(libraryId: $libraryId) {
    config {
      grey16
      grey16Exclude
      pdfOcr
      pdfShrink
      grey16Since
      pdfOcrSince
      pdfShrinkSince
    }
    pdfWorker {
      alive
      state
      version
      lastSeen
    }
    counts {
      kind
      status
      count
    }
    bytesSaved
  }
}
    `) as unknown as TypedDocumentString<LibraryPostProcessingQuery, LibraryPostProcessingQueryVariables>;
export const UpdateLibraryPostProcessingDocument = new TypedDocumentString(`
    mutation UpdateLibraryPostProcessing($libraryId: ID!, $input: PostProcessingConfigInput!) {
  updateLibraryPostProcessing(libraryId: $libraryId, input: $input) {
    bytesSaved
  }
}
    `) as unknown as TypedDocumentString<UpdateLibraryPostProcessingMutation, UpdateLibraryPostProcessingMutationVariables>;
export const RunPostProcessingDocument = new TypedDocumentString(`
    mutation RunPostProcessing($libraryId: ID!, $grey16: Boolean!, $pdfOcr: Boolean!, $pdfShrink: Boolean!, $dryRun: Boolean!) {
  runPostProcessing(
    libraryId: $libraryId
    grey16: $grey16
    pdfOcr: $pdfOcr
    pdfShrink: $pdfShrink
    dryRun: $dryRun
  )
}
    `) as unknown as TypedDocumentString<RunPostProcessingMutation, RunPostProcessingMutationVariables>;
export const ScanHistorySectionClearHistoryDocument = new TypedDocumentString(`
    mutation ScanHistorySectionClearHistory($id: ID!) {
  clearScanHistory(id: $id)
}
    `) as unknown as TypedDocumentString<ScanHistorySectionClearHistoryMutation, ScanHistorySectionClearHistoryMutationVariables>;
export const ScanHistoryTableDocument = new TypedDocumentString(`
    query ScanHistoryTable($id: ID!) {
  libraryById(id: $id) {
    id
    scanHistory {
      id
      jobId
      timestamp
      options
    }
  }
}
    `) as unknown as TypedDocumentString<ScanHistoryTableQuery, ScanHistoryTableQueryVariables>;
export const ScanRecordInspectorJobsDocument = new TypedDocumentString(`
    query ScanRecordInspectorJobs($id: ID!, $loadLogs: Boolean!) {
  jobById(id: $id) {
    id
    outputData {
      __typename
      ... on LibraryScanOutput {
        totalFiles
        totalDirectories
        ignoredFiles
        skippedFiles
        ignoredDirectories
        createdMedia
        updatedMedia
        createdSeries
        updatedSeries
      }
    }
    logs @include(if: $loadLogs) {
      id
    }
  }
}
    `) as unknown as TypedDocumentString<ScanRecordInspectorJobsQuery, ScanRecordInspectorJobsQueryVariables>;
export const DeleteLibraryThumbnailsDocument = new TypedDocumentString(`
    mutation DeleteLibraryThumbnails($id: ID!) {
  deleteLibraryThumbnails(id: $id)
}
    `) as unknown as TypedDocumentString<DeleteLibraryThumbnailsMutation, DeleteLibraryThumbnailsMutationVariables>;
export const LibraryThumbnailSelectorUpdateDocument = new TypedDocumentString(`
    mutation LibraryThumbnailSelectorUpdate($id: ID!, $input: UpdateThumbnailInput!) {
  updateLibraryThumbnail(id: $id, input: $input) {
    id
    thumbnail {
      url
    }
  }
}
    `) as unknown as TypedDocumentString<LibraryThumbnailSelectorUpdateMutation, LibraryThumbnailSelectorUpdateMutationVariables>;
export const LibraryThumbnailSelectorUploadDocument = new TypedDocumentString(`
    mutation LibraryThumbnailSelectorUpload($id: ID!, $file: Upload!) {
  uploadLibraryThumbnail(id: $id, file: $file) {
    id
    thumbnail {
      url
    }
  }
}
    `) as unknown as TypedDocumentString<LibraryThumbnailSelectorUploadMutation, LibraryThumbnailSelectorUploadMutationVariables>;
export const ProcessLibraryThumbnailsDocument = new TypedDocumentString(`
    mutation ProcessLibraryThumbnails($id: ID!, $forceRegenerate: Boolean!) {
  processLibraryThumbnails(id: $id, forceRegenerate: $forceRegenerate)
}
    `) as unknown as TypedDocumentString<ProcessLibraryThumbnailsMutation, ProcessLibraryThumbnailsMutationVariables>;
export const RegenerateThumbnailsDocument = new TypedDocumentString(`
    mutation RegenerateThumbnails($id: ID!, $forceRegenerate: Boolean!) {
  generateLibraryThumbnails(id: $id, forceRegenerate: $forceRegenerate)
}
    `) as unknown as TypedDocumentString<RegenerateThumbnailsMutation, RegenerateThumbnailsMutationVariables>;
export const ReadingListSceneDocument = new TypedDocumentString(`
    query ReadingListScene($id: ID!) {
  readingListById(id: $id) {
    id
    name
    description
    creatingUserId
    bookCount
    books {
      id
      resolvedName
      ...BookCard
    }
  }
}
    fragment BookCard on Media {
  id
  resolvedName
  extension
  pages
  size
  status
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
  readProgress {
    percentageCompleted
    page
    updatedAt
    locator {
      href
    }
  }
  readHistory {
    __typename
    completedAt
  }
  createdAt
  libraryConfig {
    skipBookOverview
  }
  metadata {
    number
    rating
    ageRating
    language
  }
}`) as unknown as TypedDocumentString<ReadingListSceneQuery, ReadingListSceneQueryVariables>;
export const ReadingListsSceneDocument = new TypedDocumentString(`
    query ReadingListsScene {
  readingLists(pagination: {none: {unpaginated: true}}) {
    nodes {
      id
      name
      creatingUserId
      ...ReadingListGridItem
    }
  }
}
    fragment ReadingListGridItem on ReadingList {
  id
  name
  bookCount
  books(limit: 5) {
    id
    thumbnail {
      url
      metadata {
        averageColor
        colors {
          color
          percentage
        }
        thumbhash
      }
    }
  }
}`) as unknown as TypedDocumentString<ReadingListsSceneQuery, ReadingListsSceneQueryVariables>;
export const SeriesSettingsSceneDocument = new TypedDocumentString(`
    query SeriesSettingsScene($id: ID!) {
  seriesById(id: $id) {
    id
    tags {
      id
      name
    }
    metadata {
      ...SeriesMetadataEditor
    }
  }
}
    fragment SeriesMetadataEditor on SeriesMetadata {
  ageRating
  booktype
  characters
  collects {
    series
    comicid
    issueid
    issues
  }
  comicImage
  comicid
  descriptionFormatted
  genres
  imprint
  links
  metaType
  publicationRun
  publisher
  status
  summary
  title
  totalIssues
  volume
  writers
  year
  lockedFields
}`) as unknown as TypedDocumentString<SeriesSettingsSceneQuery, SeriesSettingsSceneQueryVariables>;
export const SeriesSettingsSceneResetMetadataDocument = new TypedDocumentString(`
    mutation SeriesSettingsSceneResetMetadata($id: ID!, $impact: MetadataResetImpact!) {
  resetSeriesMetadata(id: $id, impact: $impact) {
    id
  }
}
    `) as unknown as TypedDocumentString<SeriesSettingsSceneResetMetadataMutation, SeriesSettingsSceneResetMetadataMutationVariables>;
export const ApiKeyTableDocument = new TypedDocumentString(`
    query APIKeyTable {
  apiKeys {
    id
    name
    permissions {
      __typename
      ... on UserPermissionStruct {
        value
      }
    }
    lastUsedAt
    expiresAt
    createdAt
  }
}
    `) as unknown as TypedDocumentString<ApiKeyTableQuery, ApiKeyTableQueryVariables>;
export const CreateApiKeyModalDocument = new TypedDocumentString(`
    mutation CreateAPIKeyModal($input: ApikeyInput!) {
  createApiKey(input: $input) {
    apiKey {
      id
    }
    secret
  }
}
    `) as unknown as TypedDocumentString<CreateApiKeyModalMutation, CreateApiKeyModalMutationVariables>;
export const DeleteApiKeyConfirmModalDocument = new TypedDocumentString(`
    mutation DeleteAPIKeyConfirmModal($id: Int!) {
  deleteApiKey(id: $id) {
    id
  }
}
    `) as unknown as TypedDocumentString<DeleteApiKeyConfirmModalMutation, DeleteApiKeyConfirmModalMutationVariables>;
export const UploadUserAvatarDocument = new TypedDocumentString(`
    mutation UploadUserAvatar($file: Upload!) {
  uploadUserAvatar(upload: $file) {
    id
    avatar {
      url
    }
  }
}
    `) as unknown as TypedDocumentString<UploadUserAvatarMutation, UploadUserAvatarMutationVariables>;
export const DeleteUserAvatarDocument = new TypedDocumentString(`
    mutation DeleteUserAvatar {
  deleteUserAvatar {
    id
    avatar {
      url
    }
  }
}
    `) as unknown as TypedDocumentString<DeleteUserAvatarMutation, DeleteUserAvatarMutationVariables>;
export const UpdateUserProfileFormDocument = new TypedDocumentString(`
    mutation UpdateUserProfileForm($input: UpdateUserInput!) {
  updateViewer(input: $input) {
    id
    username
  }
}
    `) as unknown as TypedDocumentString<UpdateUserProfileFormMutation, UpdateUserProfileFormMutationVariables>;
export const NavigationArrangementDocument = new TypedDocumentString(`
    query NavigationArrangement {
  me {
    preferences {
      navigationArrangement {
        locked
        sections {
          __typename
          config {
            __typename
            ... on SystemArrangementConfig {
              variant
              links
            }
          }
          visible
        }
      }
    }
  }
}
    `) as unknown as TypedDocumentString<NavigationArrangementQuery, NavigationArrangementQueryVariables>;
export const NavigationArrangementUpdateDocument = new TypedDocumentString(`
    mutation NavigationArrangementUpdate($input: NavigationArrangementInput!) {
  updateNavigationArrangement(input: $input) {
    __typename
  }
}
    `) as unknown as TypedDocumentString<NavigationArrangementUpdateMutation, NavigationArrangementUpdateMutationVariables>;
export const NavigationArrangementUpdateLockStatusDocument = new TypedDocumentString(`
    mutation NavigationArrangementUpdateLockStatus($locked: Boolean!) {
  updateNavigationArrangementLock(locked: $locked) {
    __typename
  }
}
    `) as unknown as TypedDocumentString<NavigationArrangementUpdateLockStatusMutation, NavigationArrangementUpdateLockStatusMutationVariables>;
export const DeletedBooksDocument = new TypedDocumentString(`
    query DeletedBooks($libraryId: ID) {
  deletedBooks(libraryId: $libraryId) {
    deletedAt
    deletedBy
    expiresAt
    purgeAt
    libraryId
    libraryName
    relativePath
    fileExists
    book {
      id
      resolvedName
      extension
      size
      thumbnail {
        url
      }
      metadata {
        writers
      }
    }
  }
}
    `) as unknown as TypedDocumentString<DeletedBooksQuery, DeletedBooksQueryVariables>;
export const DeletedBooksSettingsDocument = new TypedDocumentString(`
    query DeletedBooksSettings {
  deletedBooksSettings {
    retentionDays
    nextPurgeAt
    lastPurgeAt
    lastPurge {
      finishedAt
      jobId
      purged
      missingFiles
      failed
      originals
      errors {
        mediaId
        title
        error
      }
    }
  }
}
    `) as unknown as TypedDocumentString<DeletedBooksSettingsQuery, DeletedBooksSettingsQueryVariables>;
export const PurgedBooksDocument = new TypedDocumentString(`
    query PurgedBooks($limit: Int!) {
  purgedBooks(limit: $limit) {
    id
    mediaId
    title
    authors
    libraryName
    originalPath
    bytes
    purgedBy
    automatic
    purgedAt
  }
}
    `) as unknown as TypedDocumentString<PurgedBooksQuery, PurgedBooksQueryVariables>;
export const HeldFilesDocument = new TypedDocumentString(`
    query HeldFiles($libraryId: ID) {
  heldFiles(libraryId: $libraryId) {
    id
    reason
    steps
    title
    libraryId
    libraryName
    relativePath
    bytes
    heldAt
    heldBy
    expiresAt
    purgeAt
    canPutBack
    book {
      id
      resolvedName
      thumbnail {
        url
      }
    }
  }
}
    `) as unknown as TypedDocumentString<HeldFilesQuery, HeldFilesQueryVariables>;
export const PutBackHeldFileDocument = new TypedDocumentString(`
    mutation PutBackHeldFile($id: Int!) {
  putBackHeldFile(id: $id) {
    book {
      id
      resolvedName
    }
  }
}
    `) as unknown as TypedDocumentString<PutBackHeldFileMutation, PutBackHeldFileMutationVariables>;
export const DeleteHeldFilesDocument = new TypedDocumentString(`
    mutation DeleteHeldFiles($ids: [Int!]!) {
  deleteHeldFiles(ids: $ids) {
    deleted
    freedBytes
    kept {
      id
      title
      error
    }
  }
}
    `) as unknown as TypedDocumentString<DeleteHeldFilesMutation, DeleteHeldFilesMutationVariables>;
export const DeletedBooksLibrariesDocument = new TypedDocumentString(`
    query DeletedBooksLibraries {
  libraries(pagination: {none: {unpaginated: true}}) {
    nodes {
      id
      name
    }
  }
}
    `) as unknown as TypedDocumentString<DeletedBooksLibrariesQuery, DeletedBooksLibrariesQueryVariables>;
export const RestoreDeletedBooksDocument = new TypedDocumentString(`
    mutation RestoreDeletedBooks($ids: [ID!]!) {
  restoreDeletedBooks(ids: $ids)
}
    `) as unknown as TypedDocumentString<RestoreDeletedBooksMutation, RestoreDeletedBooksMutationVariables>;
export const PurgeDeletedBooksDocument = new TypedDocumentString(`
    mutation PurgeDeletedBooks($ids: [ID!]!) {
  purgeDeletedBooks(ids: $ids) {
    purged
    missingFiles
    freedBytes
    kept {
      mediaId
      title
      error
    }
  }
}
    `) as unknown as TypedDocumentString<PurgeDeletedBooksMutation, PurgeDeletedBooksMutationVariables>;
export const EmptyDeletedBooksDocument = new TypedDocumentString(`
    mutation EmptyDeletedBooks($libraryId: ID) {
  emptyDeletedBooks(libraryId: $libraryId) {
    purged
    missingFiles
    freedBytes
    kept {
      mediaId
      title
      error
    }
  }
}
    `) as unknown as TypedDocumentString<EmptyDeletedBooksMutation, EmptyDeletedBooksMutationVariables>;
export const UpdateDeletedBooksRetentionDocument = new TypedDocumentString(`
    mutation UpdateDeletedBooksRetention($days: Int!) {
  updateDeletedBooksRetention(days: $days) {
    retentionDays
  }
}
    `) as unknown as TypedDocumentString<UpdateDeletedBooksRetentionMutation, UpdateDeletedBooksRetentionMutationVariables>;
export const CreateEmailerSceneEmailersDocument = new TypedDocumentString(`
    query CreateEmailerSceneEmailers {
  emailers {
    name
  }
}
    `) as unknown as TypedDocumentString<CreateEmailerSceneEmailersQuery, CreateEmailerSceneEmailersQueryVariables>;
export const CreateEmailerSceneCreateEmailerDocument = new TypedDocumentString(`
    mutation CreateEmailerSceneCreateEmailer($input: EmailerInput!) {
  createEmailer(input: $input) {
    id
  }
}
    `) as unknown as TypedDocumentString<CreateEmailerSceneCreateEmailerMutation, CreateEmailerSceneCreateEmailerMutationVariables>;
export const EditEmailerSceneDocument = new TypedDocumentString(`
    query EditEmailerScene($id: Int!) {
  emailers {
    name
  }
  emailerById(id: $id) {
    id
    ...CreateOrUpdateEmailerForm
  }
}
    fragment CreateOrUpdateEmailerForm on Emailer {
  name
  isPrimary
  smtpHost
  smtpPort
  maxAttachmentSizeBytes
  senderDisplayName
  senderEmail
  tlsEnabled
  username
}`) as unknown as TypedDocumentString<EditEmailerSceneQuery, EditEmailerSceneQueryVariables>;
export const EditEmailerSceneEditEmailerDocument = new TypedDocumentString(`
    mutation EditEmailerSceneEditEmailer($id: Int!, $input: EmailerInput!) {
  updateEmailer(id: $id, input: $input) {
    id
  }
}
    `) as unknown as TypedDocumentString<EditEmailerSceneEditEmailerMutation, EditEmailerSceneEditEmailerMutationVariables>;
export const CreateOrUpdateDeviceModalCreateEmailDeviceDocument = new TypedDocumentString(`
    mutation CreateOrUpdateDeviceModalCreateEmailDevice($input: EmailDeviceInput!) {
  createEmailDevice(input: $input) {
    id
    name
  }
}
    `) as unknown as TypedDocumentString<CreateOrUpdateDeviceModalCreateEmailDeviceMutation, CreateOrUpdateDeviceModalCreateEmailDeviceMutationVariables>;
export const CreateOrUpdateDeviceModalUpdateEmailDeviceDocument = new TypedDocumentString(`
    mutation CreateOrUpdateDeviceModalUpdateEmailDevice($id: Int!, $input: EmailDeviceInput!) {
  updateEmailDevice(id: $id, input: $input) {
    id
    name
    forbidden
  }
}
    `) as unknown as TypedDocumentString<CreateOrUpdateDeviceModalUpdateEmailDeviceMutation, CreateOrUpdateDeviceModalUpdateEmailDeviceMutationVariables>;
export const DeleteDeviceConfirmationDeleteEmailDeviceDocument = new TypedDocumentString(`
    mutation DeleteDeviceConfirmationDeleteEmailDevice($id: Int!) {
  deleteEmailDevice(id: $id) {
    id
  }
}
    `) as unknown as TypedDocumentString<DeleteDeviceConfirmationDeleteEmailDeviceMutation, DeleteDeviceConfirmationDeleteEmailDeviceMutationVariables>;
export const EmailDevicesTableDocument = new TypedDocumentString(`
    query EmailDevicesTable {
  emailDevices {
    id
    name
    email
    forbidden
  }
}
    `) as unknown as TypedDocumentString<EmailDevicesTableQuery, EmailDevicesTableQueryVariables>;
export const DeleteEmailerDocument = new TypedDocumentString(`
    mutation DeleteEmailer($emailerId: Int!) {
  deleteEmailer(id: $emailerId) {
    id
  }
}
    `) as unknown as TypedDocumentString<DeleteEmailerMutation, DeleteEmailerMutationVariables>;
export const EmailerSendHistoryDocument = new TypedDocumentString(`
    query EmailerSendHistory($id: Int!, $fetchUser: Boolean!) {
  emailerById(id: $id) {
    sendHistory {
      sentAt
      recipientEmail
      sentByUserId
      sentBy @include(if: $fetchUser) {
        id
        username
      }
      attachmentMeta {
        filename
        mediaId
        media {
          resolvedName
        }
        size
      }
    }
  }
}
    `) as unknown as TypedDocumentString<EmailerSendHistoryQuery, EmailerSendHistoryQueryVariables>;
export const EmailersListDocument = new TypedDocumentString(`
    query EmailersList {
  emailers {
    id
    ...EmailerListItem
  }
}
    fragment EmailerListItem on Emailer {
  id
  name
  isPrimary
  smtpHost
  smtpPort
  lastUsedAt
  maxAttachmentSizeBytes
  senderDisplayName
  senderEmail
  tlsEnabled
  username
}`) as unknown as TypedDocumentString<EmailersListQuery, EmailersListQueryVariables>;
export const TestEmailerDocument = new TypedDocumentString(`
    mutation TestEmailer($config: EmailerClientConfig!, $recipient: String!) {
  testEmailer(config: $config, recipient: $recipient)
}
    `) as unknown as TypedDocumentString<TestEmailerMutation, TestEmailerMutationVariables>;
export const ServerEmojisSectionDocument = new TypedDocumentString(`
    query ServerEmojisSection {
  customEmojis {
    id
    name
    isAnimated
    url
  }
}
    `) as unknown as TypedDocumentString<ServerEmojisSectionQuery, ServerEmojisSectionQueryVariables>;
export const ServerEmojisSectionUploadEmojiDocument = new TypedDocumentString(`
    mutation ServerEmojisSectionUploadEmoji($input: CreateCustomEmojiInput!, $upload: Upload!) {
  uploadCustomEmoji(input: $input, upload: $upload) {
    id
    name
    isAnimated
    url
  }
}
    `) as unknown as TypedDocumentString<ServerEmojisSectionUploadEmojiMutation, ServerEmojisSectionUploadEmojiMutationVariables>;
export const ServerEmojisSectionRenameEmojiDocument = new TypedDocumentString(`
    mutation ServerEmojisSectionRenameEmoji($id: ID!, $input: UpdateCustomEmojiInput!) {
  updateCustomEmoji(id: $id, input: $input) {
    id
    name
    isAnimated
    url
  }
}
    `) as unknown as TypedDocumentString<ServerEmojisSectionRenameEmojiMutation, ServerEmojisSectionRenameEmojiMutationVariables>;
export const ServerEmojisSectionDeleteEmojiDocument = new TypedDocumentString(`
    mutation ServerEmojisSectionDeleteEmoji($id: ID!) {
  deleteCustomEmoji(id: $id)
}
    `) as unknown as TypedDocumentString<ServerEmojisSectionDeleteEmojiMutation, ServerEmojisSectionDeleteEmojiMutationVariables>;
export const ServerPublicUrlUpdateDocument = new TypedDocumentString(`
    mutation ServerPublicURLUpdate($publicUrl: String!) {
  updatePublicUrl(publicUrl: $publicUrl) {
    publicUrl
  }
}
    `) as unknown as TypedDocumentString<ServerPublicUrlUpdateMutation, ServerPublicUrlUpdateMutationVariables>;
export const ServerPublicUrlDocument = new TypedDocumentString(`
    query ServerPublicURL {
  serverConfig {
    publicUrl
  }
}
    `) as unknown as TypedDocumentString<ServerPublicUrlQuery, ServerPublicUrlQueryVariables>;
export const ServerStatsDocument = new TypedDocumentString(`
    query ServerStats {
  numberOfLibraries
  numberOfSeries
  mediaCount
  mediaDiskUsage
}
    `) as unknown as TypedDocumentString<ServerStatsQuery, ServerStatsQueryVariables>;
export const CreateScheduledJobDocument = new TypedDocumentString(`
    mutation CreateScheduledJob($input: CreateScheduledJobInput!) {
  createScheduledJob(input: $input) {
    ...ScheduledJobRow
  }
}
    fragment ScheduledJobRow on ScheduledJob {
  id
  name
  kind
  schedule
  config
  enabled
  createdAt
  lastRunAt
}`) as unknown as TypedDocumentString<CreateScheduledJobMutation, CreateScheduledJobMutationVariables>;
export const UpdateScheduledJobDocument = new TypedDocumentString(`
    mutation UpdateScheduledJob($id: Int!, $input: UpdateScheduledJobInput!) {
  updateScheduledJob(id: $id, input: $input) {
    ...ScheduledJobRow
  }
}
    fragment ScheduledJobRow on ScheduledJob {
  id
  name
  kind
  schedule
  config
  enabled
  createdAt
  lastRunAt
}`) as unknown as TypedDocumentString<UpdateScheduledJobMutation, UpdateScheduledJobMutationVariables>;
export const DeleteJobHistoryConfirmationDocument = new TypedDocumentString(`
    mutation DeleteJobHistoryConfirmation {
  deleteJobHistory {
    affectedRows
  }
}
    `) as unknown as TypedDocumentString<DeleteJobHistoryConfirmationMutation, DeleteJobHistoryConfirmationMutationVariables>;
export const JobActionMenuCancelJobDocument = new TypedDocumentString(`
    mutation JobActionMenuCancelJob($id: ID!) {
  cancelJob(id: $id)
}
    `) as unknown as TypedDocumentString<JobActionMenuCancelJobMutation, JobActionMenuCancelJobMutationVariables>;
export const JobActionMenuDeleteJobDocument = new TypedDocumentString(`
    mutation JobActionMenuDeleteJob($id: ID!) {
  cancelJob(id: $id)
}
    `) as unknown as TypedDocumentString<JobActionMenuDeleteJobMutation, JobActionMenuDeleteJobMutationVariables>;
export const JobActionMenuDeleteLogsDocument = new TypedDocumentString(`
    mutation JobActionMenuDeleteLogs($id: ID!) {
  deleteJobLogs(id: $id) {
    affectedRows
  }
}
    `) as unknown as TypedDocumentString<JobActionMenuDeleteLogsMutation, JobActionMenuDeleteLogsMutationVariables>;
export const JobDataInspectorLogsDocument = new TypedDocumentString(`
    query JobDataInspectorLogs($id: String!) {
  logs(filter: {jobId: {eq: $id}}, pagination: {none: {unpaginated: true}}) {
    nodes {
      id
      level
      message
      timestamp
    }
  }
}
    `) as unknown as TypedDocumentString<JobDataInspectorLogsQuery, JobDataInspectorLogsQueryVariables>;
export const ScheduledJobsDocument = new TypedDocumentString(`
    query ScheduledJobs {
  libraries(pagination: {none: {unpaginated: true}}) {
    nodes {
      id
      name
      emoji
    }
  }
  scheduledJobs {
    id
    name
    ...ScheduledJobRow
  }
}
    fragment ScheduledJobRow on ScheduledJob {
  id
  name
  kind
  schedule
  config
  enabled
  createdAt
  lastRunAt
}`) as unknown as TypedDocumentString<ScheduledJobsQuery, ScheduledJobsQueryVariables>;
export const DeleteScheduledJobDocument = new TypedDocumentString(`
    mutation DeleteScheduledJob($id: Int!) {
  deleteScheduledJob(id: $id)
}
    `) as unknown as TypedDocumentString<DeleteScheduledJobMutation, DeleteScheduledJobMutationVariables>;
export const JobTableDocument = new TypedDocumentString(`
    query JobTable($pagination: Pagination!) {
  jobs(pagination: $pagination) {
    nodes {
      id
      name
      description
      status
      createdAt
      completedAt
      msElapsed
      outputData {
        ...JobDataInspector
      }
      logCount
    }
    pageInfo {
      __typename
      ... on OffsetPaginationInfo {
        currentPage
        totalPages
        pageSize
        pageOffset
        zeroBased
      }
    }
  }
}
    fragment JobDataInspector on CoreJobOutput {
  __typename
  ... on LibraryScanOutput {
    totalFiles
    totalDirectories
    ignoredFiles
    skippedFiles
    ignoredDirectories
    createdMedia
    updatedMedia
    createdSeries
    updatedSeries
  }
  ... on SeriesScanOutput {
    totalFiles
    ignoredFiles
    skippedFiles
    createdMedia
    updatedMedia
  }
  ... on ThumbnailGenerationOutput {
    visitedFiles
    skippedFiles
    generatedThumbnails
    removedThumbnails
  }
  ... on TrashPurgeOutput {
    purgedBooks
    missingBookFiles
    keptBooks
    purgedHeldFiles
    freedBytes
  }
  ... on RescanBooksOutput {
    rescannedBooks
    updatedBooks
    missingBooks
  }
}`) as unknown as TypedDocumentString<JobTableQuery, JobTableQueryVariables>;
export const LiveLogsFeedDocument = new TypedDocumentString(`
    subscription LiveLogsFeed {
  tailLogFile
}
    `) as unknown as TypedDocumentString<LiveLogsFeedSubscription, LiveLogsFeedSubscriptionVariables>;
export const DeleteLogsDocument = new TypedDocumentString(`
    mutation DeleteLogs {
  deleteLogs {
    deleted
  }
}
    `) as unknown as TypedDocumentString<DeleteLogsMutation, DeleteLogsMutationVariables>;
export const PersistedLogsDocument = new TypedDocumentString(`
    query PersistedLogs($filter: LogFilterInput!, $pagination: Pagination!, $orderBy: [LogModelOrderBy!]!) {
  logs(filter: $filter, pagination: $pagination, orderBy: $orderBy) {
    nodes {
      id
      timestamp
      level
      message
      jobId
      context
    }
    pageInfo {
      __typename
      ... on OffsetPaginationInfo {
        totalPages
        currentPage
        pageSize
        pageOffset
        pageOffset
        zeroBased
      }
    }
  }
}
    `) as unknown as TypedDocumentString<PersistedLogsQuery, PersistedLogsQueryVariables>;
export const OnlineLookupSettingsDocument = new TypedDocumentString(`
    query OnlineLookupSettings {
  onlineLookupSettings {
    googleBooksApiKeySet
  }
}
    `) as unknown as TypedDocumentString<OnlineLookupSettingsQuery, OnlineLookupSettingsQueryVariables>;
export const UpdateOnlineLookupSettingsDocument = new TypedDocumentString(`
    mutation UpdateOnlineLookupSettings($googleBooksApiKey: String) {
  updateOnlineLookupSettings(googleBooksApiKey: $googleBooksApiKey) {
    googleBooksApiKeySet
  }
}
    `) as unknown as TypedDocumentString<UpdateOnlineLookupSettingsMutation, UpdateOnlineLookupSettingsMutationVariables>;
export const CreateProviderDialogCreateProviderDocument = new TypedDocumentString(`
    mutation CreateProviderDialogCreateProvider($input: CreateMetadataProviderConfigInput!) {
  createMetadataProvider(input: $input) {
    id
    providerType
    enabled
  }
}
    `) as unknown as TypedDocumentString<CreateProviderDialogCreateProviderMutation, CreateProviderDialogCreateProviderMutationVariables>;
export const EditProviderDialogDocument = new TypedDocumentString(`
    mutation EditProviderDialog($id: Int!, $input: PatchMetadataProviderConfigInput!) {
  updateMetadataProvider(id: $id, input: $input) {
    id
    ...ExistingProviderCard
  }
}
    fragment EditProviderDialog on MetadataProviderConfigModel {
  id
  providerType
  enabled
  apiTokenExpiresAt
  autoApplyConfig
}
fragment ExistingProviderCard on MetadataProviderConfigModel {
  id
  providerType
  enabled
  apiTokenExpiresAt
  autoApplyConfig
  createdAt
  updatedAt
  ...EditProviderDialog
}`) as unknown as TypedDocumentString<EditProviderDialogMutation, EditProviderDialogMutationVariables>;
export const DeleteProviderDialogDocument = new TypedDocumentString(`
    mutation DeleteProviderDialog($id: Int!) {
  deleteMetadataProvider(id: $id) {
    id
  }
}
    `) as unknown as TypedDocumentString<DeleteProviderDialogMutation, DeleteProviderDialogMutationVariables>;
export const MetadataFetchSettingsDocument = new TypedDocumentString(`
    query MetadataFetchSettings {
  metadataFetchSettings {
    fetchAfterScan
  }
}
    `) as unknown as TypedDocumentString<MetadataFetchSettingsQuery, MetadataFetchSettingsQueryVariables>;
export const UpdateMetadataFetchSettingsDocument = new TypedDocumentString(`
    mutation UpdateMetadataFetchSettings($fetchAfterScan: Boolean!) {
  updateMetadataFetchSettings(fetchAfterScan: $fetchAfterScan) {
    fetchAfterScan
  }
}
    `) as unknown as TypedDocumentString<UpdateMetadataFetchSettingsMutation, UpdateMetadataFetchSettingsMutationVariables>;
export const ProviderApiKeyInputValidateKeyDocument = new TypedDocumentString(`
    mutation ProviderApiKeyInputValidateKey($config: ValidateMetadataProviderConfigInput!) {
  validateProviderConfig(config: $config) {
    isValid
    error
    responseStatus
  }
}
    `) as unknown as TypedDocumentString<ProviderApiKeyInputValidateKeyMutation, ProviderApiKeyInputValidateKeyMutationVariables>;
export const ProvidersSectionGetProvidersDocument = new TypedDocumentString(`
    query ProvidersSectionGetProviders {
  metadataProviderConfigs {
    id
    ...ExistingProviderCard
  }
}
    fragment EditProviderDialog on MetadataProviderConfigModel {
  id
  providerType
  enabled
  apiTokenExpiresAt
  autoApplyConfig
}
fragment ExistingProviderCard on MetadataProviderConfigModel {
  id
  providerType
  enabled
  apiTokenExpiresAt
  autoApplyConfig
  createdAt
  updatedAt
  ...EditProviderDialog
}`) as unknown as TypedDocumentString<ProvidersSectionGetProvidersQuery, ProvidersSectionGetProvidersQueryVariables>;
export const AdminRecentlyAddedBooksDocument = new TypedDocumentString(`
    query AdminRecentlyAddedBooks($userId: ID, $limit: Int! = 50) {
  recentlyAddedBooks(userId: $userId, limit: $limit) {
    itemId
    userId
    username
    title
    authors
    series
    paths
    libraryId
    libraryName
    filedAt
    mediaIds
  }
}
    `) as unknown as TypedDocumentString<AdminRecentlyAddedBooksQuery, AdminRecentlyAddedBooksQueryVariables>;
export const CreateTagModalDocument = new TypedDocumentString(`
    mutation CreateTagModal($tags: [String!]!) {
  createTags(tags: $tags) {
    id
    name
  }
}
    `) as unknown as TypedDocumentString<CreateTagModalMutation, CreateTagModalMutationVariables>;
export const DeleteTagConfirmModalDocument = new TypedDocumentString(`
    mutation DeleteTagConfirmModal($tags: [String!]!) {
  deleteTags(tags: $tags) {
    id
    name
  }
}
    `) as unknown as TypedDocumentString<DeleteTagConfirmModalMutation, DeleteTagConfirmModalMutationVariables>;
export const RenameTagModalDocument = new TypedDocumentString(`
    mutation RenameTagModal($id: Int!, $name: String!) {
  renameTag(id: $id, name: $name) {
    id
    name
  }
}
    `) as unknown as TypedDocumentString<RenameTagModalMutation, RenameTagModalMutationVariables>;
export const TagTableDocument = new TypedDocumentString(`
    query TagTable {
  tags {
    id
    name
  }
}
    `) as unknown as TypedDocumentString<TagTableQuery, TagTableQueryVariables>;
export const UserStatsDocument = new TypedDocumentString(`
    query UserStats {
  userCount
  topReaders(take: 1) {
    id
    username
    finishedReadingSessionsCount
  }
  activeReadingSessionCount
  finishedReadingSessionCount
}
    `) as unknown as TypedDocumentString<UserStatsQuery, UserStatsQueryVariables>;
export const CreateOrUpdateUserFormUpdateUserDocument = new TypedDocumentString(`
    mutation CreateOrUpdateUserFormUpdateUser($id: ID!, $input: UpdateUserInput!) {
  updateUser(id: $id, input: $input) {
    id
    username
    ageRestriction {
      age
      restrictOnUnset
    }
    permissions
    maxSessionsAllowed
  }
}
    `) as unknown as TypedDocumentString<CreateOrUpdateUserFormUpdateUserMutation, CreateOrUpdateUserFormUpdateUserMutationVariables>;
export const CreateOrUpdateUserFormCreateUserDocument = new TypedDocumentString(`
    mutation CreateOrUpdateUserFormCreateUser($input: CreateUserInput!) {
  createUser(input: $input) {
    id
  }
}
    `) as unknown as TypedDocumentString<CreateOrUpdateUserFormCreateUserMutation, CreateOrUpdateUserFormCreateUserMutationVariables>;
export const CreateUserSceneDocument = new TypedDocumentString(`
    query CreateUserScene {
  users(pagination: {none: {unpaginated: true}}) {
    nodes {
      username
    }
  }
}
    `) as unknown as TypedDocumentString<CreateUserSceneQuery, CreateUserSceneQueryVariables>;
export const UpdateUserSceneDocument = new TypedDocumentString(`
    query UpdateUserScene($id: ID!, $skip: Boolean!) {
  me {
    id
  }
  userById(id: $id) @skip(if: $skip) {
    id
    avatarUrl
    username
    ageRestriction {
      age
      restrictOnUnset
    }
    permissions
    maxSessionsAllowed
    isServerOwner
  }
  users(pagination: {none: {unpaginated: true}}) @skip(if: $skip) {
    nodes {
      username
    }
  }
}
    `) as unknown as TypedDocumentString<UpdateUserSceneQuery, UpdateUserSceneQueryVariables>;
export const ClearLoginActivityConfirmationDocument = new TypedDocumentString(`
    mutation ClearLoginActivityConfirmation {
  deleteLoginActivity
}
    `) as unknown as TypedDocumentString<ClearLoginActivityConfirmationMutation, ClearLoginActivityConfirmationMutationVariables>;
export const LoginActivityTableDocument = new TypedDocumentString(`
    query LoginActivityTable {
  loginActivity {
    id
    ipAddress
    userAgent
    authenticationSuccessful
    timestamp
    user {
      id
      username
      avatarUrl
    }
  }
}
    `) as unknown as TypedDocumentString<LoginActivityTableQuery, LoginActivityTableQueryVariables>;
export const DeleteUserDocument = new TypedDocumentString(`
    mutation DeleteUser($id: ID!, $hardDelete: Boolean) {
  deleteUser(id: $id, hardDelete: $hardDelete) {
    id
  }
}
    `) as unknown as TypedDocumentString<DeleteUserMutation, DeleteUserMutationVariables>;
export const UserActionMenuLockUserDocument = new TypedDocumentString(`
    mutation UserActionMenuLockUser($id: ID!, $lock: Boolean!) {
  updateUserLockStatus(id: $id, lock: $lock) {
    id
    isLocked
  }
}
    `) as unknown as TypedDocumentString<UserActionMenuLockUserMutation, UserActionMenuLockUserMutationVariables>;
export const UserActionMenuDeleteUserSessionsDocument = new TypedDocumentString(`
    mutation UserActionMenuDeleteUserSessions($id: ID!) {
  deleteUserSessions(id: $id)
}
    `) as unknown as TypedDocumentString<UserActionMenuDeleteUserSessionsMutation, UserActionMenuDeleteUserSessionsMutationVariables>;
export const UserTableDocument = new TypedDocumentString(`
    query UserTable($pagination: Pagination!) {
  users(pagination: $pagination) {
    nodes {
      id
      avatarUrl
      username
      isServerOwner
      isLocked
      createdAt
      lastLogin
      loginSessionsCount
    }
    pageInfo {
      __typename
      ... on OffsetPaginationInfo {
        totalPages
        currentPage
        pageSize
        pageOffset
        zeroBased
      }
    }
  }
}
    `) as unknown as TypedDocumentString<UserTableQuery, UserTableQueryVariables>;
export const SmartListsWithSearchDocument = new TypedDocumentString(`
    query SmartListsWithSearch($input: SmartListsInput!) {
  smartLists(input: $input) {
    id
    ...SmartListWebGridItem
  }
}
    fragment SmartListWebGridItem on SmartList {
  id
  name
  books(limit: 5) {
    thumbnail {
      url
      metadata {
        averageColor
        colors {
          color
          percentage
        }
        thumbhash
      }
    }
  }
  meta {
    matchedBooks
    matchedSeries
    matchedLibraries
  }
}`) as unknown as TypedDocumentString<SmartListsWithSearchQuery, SmartListsWithSearchQueryVariables>;
export const SmartListByIdDocument = new TypedDocumentString(`
    query SmartListById($id: ID!) {
  smartListById(id: $id) {
    id
    creatorId
    description
    defaultGrouping
    filters
    joiner
    name
    visibility
    views {
      id
      listId
      name
      bookColumns {
        id
        position
      }
      bookSorting {
        id
        desc
      }
      groupColumns {
        id
        position
      }
      groupSorting {
        id
        desc
      }
      search
    }
  }
}
    `) as unknown as TypedDocumentString<SmartListByIdQuery, SmartListByIdQueryVariables>;
export const SmartListMetaDocument = new TypedDocumentString(`
    query SmartListMeta($id: ID!) {
  smartListMeta(id: $id) {
    matchedBooks
    matchedSeries
    matchedLibraries
  }
}
    `) as unknown as TypedDocumentString<SmartListMetaQuery, SmartListMetaQueryVariables>;
export const UpdateSmartListDocument = new TypedDocumentString(`
    mutation UpdateSmartList($id: ID!, $input: SaveSmartListInput!) {
  updateSmartList(id: $id, input: $input) {
    __typename
  }
}
    `) as unknown as TypedDocumentString<UpdateSmartListMutation, UpdateSmartListMutationVariables>;
export const SmartListItemsDocument = new TypedDocumentString(`
    query SmartListItems($id: ID!, $filter: MediaFilterInput, $grouping: SmartListGrouping) {
  smartListItems(id: $id, filter: $filter, grouping: $grouping) {
    __typename
    ... on SmartListGrouped {
      items {
        entity {
          __typename
          ... on Series {
            id
            name
          }
          ... on Library {
            id
            name
          }
        }
        books {
          ...BookCard
          ...BookMetadata
          path
          tags {
            id
            name
          }
        }
      }
    }
    ... on SmartListUngrouped {
      books {
        ...BookCard
        ...SmartListItemBookMetadata
        path
        tags {
          id
          name
        }
      }
    }
  }
}
    fragment BookCard on Media {
  id
  resolvedName
  extension
  pages
  size
  status
  thumbnail {
    url
    metadata {
      averageColor
      colors {
        color
        percentage
      }
      thumbhash
    }
    height
    width
  }
  readProgress {
    percentageCompleted
    page
    updatedAt
    locator {
      href
    }
  }
  readHistory {
    __typename
    completedAt
  }
  createdAt
  libraryConfig {
    skipBookOverview
  }
  metadata {
    number
    rating
    ageRating
    language
  }
}
fragment BookMetadata on Media {
  metadata {
    ageRating
    rating
    characters
    colorists
    coverArtists
    editors
    genres
    inkers
    letterers
    links
    pencillers
    publisher
    series
    teams
    writers
    year
    month
    day
    volume
    number
  }
}
fragment SmartListItemBookMetadata on Media {
  metadata {
    ageRating
    rating
    characters
    colorists
    coverArtists
    editors
    genres
    inkers
    letterers
    links
    pencillers
    publisher
    teams
    writers
    year
    month
    day
    format
    identifierAmazon
    identifierCalibre
    identifierGoogle
    identifierIsbn
    identifierMobiAsin
    identifierUuid
    language
    notes
    number
    pageCount
    series
    seriesGroup
    storyArc
    storyArcNumber
    title
    titleSort
    volume
  }
}`) as unknown as TypedDocumentString<SmartListItemsQuery, SmartListItemsQueryVariables>;
export const CreateSmartListViewDocument = new TypedDocumentString(`
    mutation CreateSmartListView($input: SaveSmartListView!) {
  createSmartListView(input: $input) {
    id
    listId
    name
    search
    enableMultiSort
    bookColumns {
      id
      position
    }
    bookSorting {
      id
      desc
    }
    groupColumns {
      id
      position
    }
    groupSorting {
      id
      desc
    }
  }
}
    `) as unknown as TypedDocumentString<CreateSmartListViewMutation, CreateSmartListViewMutationVariables>;
export const UpdateSmartListViewDocument = new TypedDocumentString(`
    mutation UpdateSmartListView($originalName: String!, $input: SaveSmartListView!) {
  updateSmartListView(originalName: $originalName, input: $input) {
    id
    listId
    name
    search
    enableMultiSort
    bookColumns {
      id
      position
    }
    bookSorting {
      id
      desc
    }
    groupColumns {
      id
      position
    }
    groupSorting {
      id
      desc
    }
  }
}
    `) as unknown as TypedDocumentString<UpdateSmartListViewMutation, UpdateSmartListViewMutationVariables>;
export const DeleteSmartListViewDocument = new TypedDocumentString(`
    mutation DeleteSmartListView($id: ID!, $name: String!) {
  deleteSmartListView(id: $id, name: $name) {
    id
    name
  }
}
    `) as unknown as TypedDocumentString<DeleteSmartListViewMutation, DeleteSmartListViewMutationVariables>;
export const SmartListBasicSettingsSceneDocument = new TypedDocumentString(`
    query SmartListBasicSettingsScene {
  smartLists(input: {mine: true}) {
    name
  }
}
    `) as unknown as TypedDocumentString<SmartListBasicSettingsSceneQuery, SmartListBasicSettingsSceneQueryVariables>;
export const DeleteSmartListDocument = new TypedDocumentString(`
    mutation DeleteSmartList($id: ID!) {
  deleteSmartList(id: $id) {
    __typename
  }
}
    `) as unknown as TypedDocumentString<DeleteSmartListMutation, DeleteSmartListMutationVariables>;
export const MyReaderSettingsDocument = new TypedDocumentString(`
    query MyReaderSettings {
  myReaderSettings
}
    `) as unknown as TypedDocumentString<MyReaderSettingsQuery, MyReaderSettingsQueryVariables>;
export const UpdateMyReaderSettingsDocument = new TypedDocumentString(`
    mutation UpdateMyReaderSettings($settings: String!) {
  updateMyReaderSettings(settings: $settings)
}
    `) as unknown as TypedDocumentString<UpdateMyReaderSettingsMutation, UpdateMyReaderSettingsMutationVariables>;
export const DirectoryListingDocument = new TypedDocumentString(`
    query DirectoryListing($input: DirectoryListingInput!, $pagination: Pagination!) {
  listDirectory(input: $input, pagination: $pagination) {
    nodes {
      parent
      files {
        name
        path
        isDirectory
        media {
          id
          resolvedName
          thumbnail {
            url
          }
          extension
        }
      }
    }
    pageInfo {
      __typename
      ... on OffsetPaginationInfo {
        currentPage
        totalPages
        pageSize
        pageOffset
        zeroBased
      }
    }
  }
}
    `) as unknown as TypedDocumentString<DirectoryListingQuery, DirectoryListingQueryVariables>;
export const UploadConfigDocument = new TypedDocumentString(`
    query UploadConfig {
  uploadConfig {
    enabled
    maxFileUploadSize
  }
}
    `) as unknown as TypedDocumentString<UploadConfigQuery, UploadConfigQueryVariables>;