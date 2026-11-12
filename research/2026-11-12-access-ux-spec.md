# Access UX redesign: Public access, approvals, friends' screens (spec, 2026-11-12)

Branch `feat/access-ux` (from `43fa3fe50`). UX lead's spec for the screens built in the public-access and
friend-onboarding projects. The owner's complaint: *"the public access is annoying: too much info, too
dispersed, instead of what would actually matter"*. Screenshots of the current state are in
`research/ux-2026-11-12/before/` (sandbox data only; link addresses, reading-app addresses, QR codes and
generated passwords are masked in the DOM before each shot). Out of scope, owned by `feat/public-login`:
the public sign-in/signup pages, the auth scene, `apps/server/src/routers/public_pages`, `deploy/public-edge`.

## 0. In brief

**Top problems**

1. *Public access* is one long page, 7,014 px on desktop (7.8 screens) and 10,506 px on a phone (12.4
   screens), with eight stacked sections, 278 strings and ~1,450 words. The first screen shows warnings and nine status numbers; the
   question "who can get in" is answered on the 6th screen (desktop) / 9th screen (phone), and the main
   action (invite someone) is a disabled button inside an always-open form.
2. One subject, four places: people, devices, device links ("Invite links") and signup links are four
   sections about the same thing, *people*. History is everywhere: every revoked device, used link and
   terminal leftover is listed with a button; *Recent sign-ins* repeats Users → Authentication history,
   mostly the owner's own LAN sign-ins, with raw user agents.
3. The warnings ("Needs a look") can't be cleared: they last while true or fade after 3 days, so the
   badge is always on and is learnt to be ignored. Two of the five are things nobody can act on (guesses at
   names that don't exist), one is broken (no name: "was refused a new device 1 times…"), and "Review"
   only jumps down the long page.
4. Jargon and raw data: "Proxy last told", "Names waiting", "Pass required", "Reset sign-in budget",
   "Made in the terminal", user-agent strings, IP addresses, link UUIDs and server-made English in
   Activity ("1 accounts turned off", "168 hours").
5. Badges: two badges stacked on a 16 px gear, the access one clipped by the bottom of the window; on a
   phone nothing shows (no count in the settings menu, nothing on the header).
6. Friends' and approval screens: *Add books* and *Book approvals* are English only (no keys at all); the
   settings menu shows the raw key `webApp.settings…` for Book approvals; *Add books* offers "Send anyway"
   for a file the server refuses as already in the library; each book is a full edit form; *My devices*
   says the count twice and shows IP addresses; *Reading apps* always shows the add form (disabled at the
   limit) and repeats the instructions in every card.

**The redesign** (details in §2–§9)

* *Public access* becomes four blocks: a **status line** (one sentence, *Details* behind a click), **Needs
  your attention** (every item one line, one action, *Dismiss*, *Clear all*; "All good" when empty),
  **People** (one list: who can sign in, who is invited, who can't; one primary button **Invite
  someone**; a person opens a side sheet with their devices, password, libraries) and **History**
  (sign-ins and changes, collapsed). Ended links and removed devices are behind *Show ended* / *Show
  removed devices*. Device links move into the person sheet. The owner's own row and own sign-ins go.
* Dismissing needs a small server change (§3): a table `access_warning_reviews`, two owner-only mutations,
  three new attention kinds (locked accounts, leftover devices, books waiting) so the list and the badge
  hold *everything* that needs the owner.
* One badge on the gear (the attention count), never clipped; a dot on the phone header; counts in the
  phone settings menu.
* Friends: *My devices* and *Reading apps* become plain lists with one action each; *Add books* gets
  translations, compact rows and the right action for duplicates; *Book approvals* gets translations and a
  collapsed per-person limits block.

Priority: **P1** Public access + server + badge (the owner's requirement). **P2** My devices, Reading apps,
nav label fix. **P3** Add books and Book approvals (translations, duplicate action, compact rows).
**P4** user editor bits.

Decisions taken here (easy to flip if the owner disagrees):

* Books waiting for approval are one attention item on Public access, and for the owner the gear shows
  one badge (attention items) instead of two; the Book approvals page keeps its own queue and count.
* "Show ended" (links, invites, devices of removed people) and "Show removed devices" (per person)
  replace a single "Show revoked": links also end by expiring or being used up.
* Guesses at names that don't exist stay an item (the owner asked for failed-sign-in bursts) but as one
  merged line, not three.
* Device links stay (they matter while devices need a pass) but only inside the person sheet.

## 1. Principles (apply to every screen)

1. **One question per screen, answered on the first view.** Public access: *who can get in, is anything
   wrong, how do I invite someone*. My devices: *which devices use my account*. Reading apps: *how do I
   read in my app*. Add books: *what do I still have to do*. Book approvals: *what waits for me*.
2. **One primary button per first view** (`Button variant="primary"`); everything else `outline`/`ghost`
   or in an overflow menu (`DropdownMenu` with a `MoreHorizontal` `IconButton`).
3. **Progressive disclosure**: details, history, ended items, technical numbers live behind a click
   (`Sheet`/`Drawer`, `Dialog`, `Accordion`, a "Show …" ghost button). Nothing is deleted from the data.
4. **Plain words**, sentence case, no product jargon (pass, budget, proxy, OPDS key) on first views; names
   of things the user knows (KOReader, Kobo, iPhone) spelled right. Relative times ("2 h ago") in rows,
   absolute time in a `ToolTip`.
5. **Phone first**: one column at 390 px, no horizontal scroll, tap targets ≥ 40 px, side sheets become
   bottom drawers (`Drawer`, as `BreadcrumbPageSwitcher` does), dialogs full width.
6. **Stump's own components and patterns** (`@stump/components`): `NewCard` + `NewCard.Row` for settings
   groups (as Preferences and the scanner settings do), `Sheet` for a detail inspector (as
   `ScanRecordInspector`), `Dialog`/`ConfirmationModal`, `Alert`, `Badge`, `Tabs`, `WideSwitch`,
   `NativeSelect`, `Input`, `ToolTip`, `toast` (sonner) with an *Undo* action. The access page's own
   `Section`/`RowList`/`Row`/`Empty` helpers may stay as thin wrappers but must look like `NewCard`.
7. **en-US and es-ES for every string** (keys in `packages/i18n/src/locales/{en-US,es-ES}.json`, the
   existing locale tests extended). Spanish is the owner's family language: short, neutral, tú.
8. **Secrets** (link addresses, reading-app addresses, QR codes, passwords) appear only in the dialog that
   made them or behind an explicit *Show address*; never in lists.

## 2. Owner: Settings → Public access (`/settings/access`)

### 2.1 Problems (with evidence)

| # | Problem | Screenshot |
|---|---------|------------|
| P1 | Page is 1440×7014 (desktop) and 390×10506 (phone). Section heights on desktop: warnings 279 px, status ~200, signup links 644, invite links 294, devices 799, **sign-in protection 2,246**, people 433, **activity 1,457**. | `owner-access-desktop.png`, `owner-access-phone.png` |
| P2 | First view = warnings + a strip of 9 numbers ("Proxy last told", "Names waiting", "Failed attempts (10 min)", "Open invites", …). No people, no primary action; on a phone the whole first screen is warnings and numbers, the "Ready" badge floats in the middle. | `owner-access-first-view-desktop.png`, `owner-access-first-view-phone.png` |
| P3 | People is the 7th section (starts ~4,700 px down on desktop, ~7,000 px on phone). Each row packs a switch, an unlabeled stepper (− 3 +), *Set a password* and *Manage*; "Devices 3 of 3 · 3 sessions · 3 libraries hidden" is a status code, not a sentence. The owner is listed too. | `owner-access-desktop.png` (People), `owner-access-phone.png` |
| P4 | Signup links and Invite links: two always-open forms with four fields each; *Create link* is disabled until a note is typed, with no message. Signup links and device links are separate sections although both are "invite someone". Used/revoked links stay in the list with buttons ("Turn off its accounts" on a used link). | `owner-access-preset-editor-*.png`, `owner-access-link-created-*.png` |
| P5 | Devices with a pass: raw user agents ("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit…"), IP addresses, "Reset sign-in budget" on every row, a "Made in the terminal" group of 6 revoked leftovers each with *Restore*. | `owner-access-desktop.png` (Devices) |
| P6 | Sign-in protection: an empty "Nobody is locked out." box, a note about "the entry proxy", and 50 recent sign-ins that are mostly the owner's own LAN sign-ins (`nitou OK … node`). The Users page already has *Authentication history* with the same rows. | `owner-access-desktop.png`, `owner-users-desktop.png` |
| P7 | Activity: 25+ rows of server-made English: "link 427be086-94a4-…", "1 accounts turned off", "single link, 1 accounts, 72 hours", "public · …". | `owner-access-desktop.png` (bottom) |
| P8 | Warnings ("Needs a look (5)"): can't be dismissed; "was refused a new device 1 times because they use all they may." has no name (the person was removed); "2 failed sign-ins on names that don't exist" and "Somebody kept guessing at a name that doesn't exist" need no action; *Review* is an in-page anchor. | `owner-access-first-view-desktop.png` |
| P9 | "Let book-curator in" dialog is a dead end: it lists the extra permissions and offers *Manage* (the user editor, where the rows are in an inner scroll box). | `owner-access-public-signin-dialog-*.png`, `owner-user-editor-permissions-*.png` |
| P10 | Set password dialog is fine; it lacks a *Copy* button before setting. | `owner-access-set-password-dialog-*.png` |

### 2.2 New structure

First view, top to bottom (desktop and phone):

```
[status line]  ✓ Open to the internet at books.example.org · 2 people can sign in      Details
[Needs your attention (3)]                                                      Clear all
   ⚠ lucia is using all 3 devices.                                  [Raise limit]   ✕
   ⚠ 7 sign-in attempts with names that don't exist. Stump slowed them down. [See attempts] ✕
   ⚠ 1 book waits for your approval.                                    [Review]     ✕
[People]  Who can sign in from the internet.                       [+ Invite someone]
   Can sign in from the internet (2)
     lucia   Internet   3 of 3 devices · last in 2 h ago                            ›
     dani    Internet   0 of 3 devices · never signed in                            ›
   Invited, not in yet (2)
     Tía Rosa      One person · expires in 3 days                                    ⋯
     Club de lectura   Several people · 0 of 5 joined · expires Nov 19               ⋯
   Can't sign in from the internet (1)  ›   (collapsed)
   Show ended (3)
[History]  Show history ›   (collapsed: Sign-ins | Changes)
```

What moves behind a click, merged or removed:

| Today | New place |
|-------|-----------|
| Status strip (9 numbers) | One status line; numbers in the **Details** dialog (§2.8) |
| Warnings strip | **Needs your attention** (§2.3), first section |
| Signup links form + list | **Invite someone** dialog (§2.5); open links as rows under *Invited, not in yet*; ended ones behind *Show ended* |
| Invite links (device links) form + list | Person sheet → *Add a device with a link* (§2.4); unused ones as rows under *Invited, not in yet*; ended behind *Show ended* |
| Devices with a pass | Person sheet → *Devices* (active only; *Show removed devices (n)*). Devices of removed people / terminal passes that still work: one attention item (*leftover devices*); revoked ones only behind *Show ended* |
| Sign-in protection → Locked right now | Attention item *ACCOUNT_LOCKED* with *Unblock*; *Unblock* also in the person sheet |
| Sign-in protection → Recent sign-ins | **History → Sign-ins** (collapsed), others only (§2.7) |
| The entry-proxy note | Details dialog |
| People (switch, stepper, buttons in the row) | People list rows (no controls) + **person sheet** (§2.4) |
| The owner's row | Removed (the owner never signs in through the public address) |
| Activity | **History → Changes** (collapsed), translated sentences, no UUIDs |

### 2.3 Needs your attention (the owner's requirement)

* **Always the first section.** Heading `Needs your attention ({n})`, at its right a ghost button *Clear
  all*. Up to 5 items shown (alerts first, then newest `lastAt` first); more behind *Show all ({n})*.
  It gathers every warning the server computes today plus what is pending elsewhere: people waiting
  (locked out, refused because they aren't allowed, books waiting for approval) and leftovers worth
  cleaning (old devices that still work). Ended things that no longer work are not items: they are only
  behind *Show ended* / *Show removed devices* (§2.4).
* **Each item is one line**: severity icon (`AlertTriangle` amber for `WARN`, `ShieldAlert` red for
  `ALERT`), one sentence ≤ 90 characters in en, **one** action button (`size="sm" variant="outline"`), and
  a *Dismiss* `IconButton` (`X`, `aria-label` and `ToolTip` "Dismiss"). On a phone the action button and
  the ✕ sit on the right of the sentence's second line; tap targets ≥ 40 px.
* **Dismiss** hides the item and shows a toast *Marked as seen. It comes back if something changes.* with
  *Undo*. **Clear all** dismisses every item shown and toasts *{n} marked as seen. History keeps them.*
  with *Undo*. Nothing is deleted: the sign-in rows and audit rows stay; each dismiss call writes one
  audit row (shown in History → Changes as *Marked as seen: …*).
* A dismissed item **comes back** when something new happens (a newer event, or a worse state: more
  devices, more failures, another extra permission). It does not come back because an old event faded.
* **Empty**: one calm line, `CheckCircle2` + *All good. Nothing needs your attention.* (muted text, no
  card), in the same place. No badge.
* The **sidebar badge counts exactly the items of this list** (`accessWarningCount`) and goes to 0 after
  *Clear all* (the mutation invalidates the count query; polling stays at 60 s).

Items (kind → sentence → action). `{name}` is the username; `{n}` the count. Plurals with i18next
`_one`/`_other`.

| Kind | en | es | Action (en / es) → what it does |
|------|----|----|------|
| `AT_DEVICE_LIMIT` | {name} is using all {n} devices. | {name} usa sus {n} dispositivos. | Raise limit / Subir límite → `setPersonAccess(maxDevices: limit+1)` (hidden at 10, the maximum), toast *{name} can now use {n} devices* + Undo |
| `OVER_DEVICE_LIMIT` | {name} has {n} devices; the limit is {limit}. | {name} tiene {n} dispositivos; el límite es {limit}. | See devices / Ver dispositivos → person sheet, Devices |
| `MANY_DEVICES` | {name} has {n} devices. | {name} tiene {n} dispositivos. | See devices / Ver dispositivos |
| `DEVICE_LIMIT_HIT` | {name} couldn't add a device: limit reached ({n}×). | {name} no pudo añadir un dispositivo: límite alcanzado ({n} veces). | Raise limit / Subir límite |
| `MANY_SESSIONS` | {name} is signed in {n} times. | {name} tiene {n} sesiones abiertas. | Sign out everywhere / Cerrar sesiones → `ConfirmationModal`, then `deleteUserSessions(id)` |
| `SIGNIN_FAILURES` (person) | {n} wrong passwords for {name} in the last hour. | {n} contraseñas incorrectas de {name} en la última hora. | Set password / Poner contraseña → `SetPasswordDialog` |
| `SIGNIN_FAILURES` (no person; merged with unknown-name `NAME_BACKOFF`, §3.2) | {n} sign-in attempts with names that don't exist. *(+ "Stump slowed them down." when a slow-down happened)* | {n} intentos con nombres que no existen. *(+ "Stump los frenó.")* | See attempts / Ver intentos → History, Sign-ins, failures only |
| `NAME_BACKOFF` (person) | Someone kept guessing {name}'s password. Stump slowed them down. | Alguien probó muchas contraseñas de {name}. Stump lo frenó. | See attempts / Ver intentos |
| `GLOBAL_BUDGET` (alert) | Too many failed sign-ins: Stump let nobody in for a few minutes. | Demasiados intentos fallidos: Stump no dejó entrar a nadie unos minutos. | See attempts / Ver intentos |
| `NOT_ENABLED_TRIED` | {name} tried to sign in from the internet but isn't allowed. | {name} intentó entrar desde internet, pero no tiene permiso. | Let in / Dejar entrar → let-in dialog (§2.6) |
| `NOT_FRIEND_CLASS` | {name} may sign in from the internet, but {extras} keeps them out. | {name} tiene permitido entrar desde internet, pero {extras} se lo impide. | Fix / Arreglar → let-in dialog with the extras |
| `LINK_NEAR_BUDGET` | The link “{note}” has 1 account left. | Al enlace «{note}» le queda 1 cuenta. | New link / Nuevo enlace → Invite dialog prefilled (same note, kind, preset) |
| `LINK_NO_LIBRARIES` | The link “{note}” gives no library: nobody can use it. | El enlace «{note}» no da ninguna biblioteca: nadie puede usarlo. | Revoke / Anular → `revokeAccessSignupLink` |
| `ACCOUNT_LOCKED` (new) | {name} is locked out after {n} wrong passwords. | {name} está bloqueado tras {n} contraseñas incorrectas. | Unblock / Desbloquear → `unblockUser(id)` |
| `LEFTOVER_DEVICES` (new) | {n} old devices still work (unused 2+ months, terminal or removed people). | {n} dispositivos viejos siguen funcionando (sin uso 2+ meses, del terminal o de personas borradas). | Remove them / Quitarlos → `ConfirmationModal` listing them, then `revokeAccessDevice` for each |
| `BOOKS_WAITING` (new) | {n} books wait for your approval. | {n} libros esperan tu aprobación. | Review / Revisar → `/settings/book-approvals` |

### 2.4 People (one list) and the person sheet

**Section** (`NewCard` with label *People*, description *Who can sign in from the internet.*, the label's
`actions` slot holds the page's only primary button **Invite someone** with `UserPlus`; on a phone the
button is full width under the description).

Groups (small muted subheadings inside the card; a group with no rows is not shown):

1. *Can sign in from the internet ({n})*: people with `publicSignin`. Row: `Avatar` + name, badges
   (*Locked* / *Bloqueado* red; *Extra permissions* / *Permisos de más* amber when `beyondFriendSet` is
   not empty, since those keep them out; *Must change password* / *Debe cambiar la contraseña* muted),
   second line *{active} of {limit} devices · last in
   {when}* or *· never signed in*. The whole row is a button (`ChevronRight` at the end) that opens the
   person sheet. No switch, stepper or buttons in the row.
2. *Invited, not in yet ({n})*: open signup links (status open) and unused device links. Row: note (or
   device label), second line *One person · expires {when}* / *Several people · {used} of {max} joined ·
   expires {when}* / *Device link for {name} · expires {when}*. Overflow (`⋯`): *Revoke* (confirmation as
   today; for signup links the existing "also turn off the accounts it made" checkbox).
3. *Can't sign in from the internet ({n})*: other non-owner, non-deleted accounts; collapsed by default
   (a ghost button with the count); rows open the person sheet, where the switch lets them in.
4. *Show ended ({n})* (ghost button at the end): revoked/expired/used-up links and invites, and revoked
   devices whose person is removed, with their date and *Turn off the accounts it made* (signup links that
   made accounts) in the overflow. Hidden again with *Hide ended*.

Order inside groups: last sign-in, newest first; invites by expiry, soonest first.

**Person sheet** (`Sheet` from the right on ≥ 768 px, size `lg`; `Drawer` from the bottom, 90 dvh, on a
phone). Title = username; description *Last in {when} from {device}* / *Never signed in*. Blocks
(`NewCard.Row`s), top to bottom:

1. **Sign in from the internet** `WideSwitch`. Help when on: *With their password, on up to {limit}
   devices.* Turning it on opens the let-in dialog (§2.6); turning it off asks *{name} won't be able to
   come in from the internet on any device.* / *{name} no podrá entrar desde internet en ningún
   dispositivo.* (`ConfirmationModal`; the devices are kept and work again when it is turned back on).
2. **Devices ({active} of {limit})**: active devices only. Row: kind icon (`Monitor` browser,
   `BookOpen` reading app, `Terminal` terminal pass), label, `Badge` for kind, second line *Used {when}*;
   tap expands *First seen {when} · {ip} · {agentLabel}*. Overflow: *Allow more tries* (the old *Reset
   sign-in budget*; help *After too many wrong passwords on this device.*), *Remove* (confirmation
   *Remove {label}? It stops working within seconds.*). Below the list: **Device limit** `NativeSelect`
   1–10 (*{n} devices*), **Add a device with a link** (ghost; reveals the old invite form inline: *Device
   name* (placeholder *Ana's phone*), *Works for* 1 h / 1 day / 3 days / 7 days / 30 days, *Can be
   opened* 1 / 3 / 5 / 10 times → *Create link* → the same result block as §2.5), **Show removed devices
   ({n})** (revoked rows with *Restore*).
3. **Password and sign-in**: *Set a new password* (the existing `SetPasswordDialog`), *Sign out
   everywhere* (`deleteUserSessions`, confirmation *{name} will have to sign in again everywhere.*); when
   locked, an `Alert` *Locked after {n} wrong passwords.* with *Unblock*.
4. **Libraries**: *Sees: Biblioteca* (or *Sees everything* / *Sees nothing*), *Change* reveals the
   library checklist (`setPersonAccess(onlyLibraryIds)`), and the switch *Hide new libraries*
   (`newLibrariesHidden`).
5. Links: *Upload limits* (→ `/settings/book-approvals?person={id}#limits`), *Edit account* (→
   `/settings/users/{id}/manage`).
6. Danger zone (ghost destructive): *Remove all devices* (`revokeAccessDevicesOfPerson`, confirmation
   *All of {name}'s devices stop working within seconds.*).

The sheet is reachable by URL: `/settings/access?person={id}` (so attention items and other pages can
link to it).

### 2.5 Invite someone (dialog)

`Dialog` (desktop) / full-screen `Drawer` (phone). Two steps in the same dialog.

Step 1 (form):

* *Who is it for?* `Input`, placeholder *e.g. Aunt Rosa*; help *Only you see this.* Required: the
  submit button stays enabled; on submit without it, the field shows *Write who it's for.*
* *Link for*: `RadioGroup` (or `Tabs` as a segmented control) *One person* (default) | *Several people*.
  With several: *How many accounts* `NativeSelect` 2–25 (default 5).
* *Works for*: 1 day, **3 days**, 7 days, 14 days, 30 days.
* *What they get*: the existing preset summary card (`access-preset-summary`) with *Change*, which
  reveals the existing editor in place (permissions as checkboxes in sentence case, libraries, devices per
  person, age limit). The open-link + Add books warning stays.
* Footer: *Cancel* (outline) · *Create link* (primary).

Step 2 (result): title *Send this link to {note}*; read-only `Input` with the address; *Copy* (primary)
and *Share* (outline; only where `navigator.share` exists, i.e. phones); *Show QR code* (ghost, toggles
the existing `QrCode`); muted note *You'll only see it now. If it gets lost, make a new one.*; footer
*Done*. Closing the dialog clears the address from memory as today.

### 2.6 Let-in dialog (switch on, *Let in*, *Fix*)

* Title *Let {name} in from the internet?*
* Body (one sentence each): *They'll sign in with their password on up to {limit} devices.* *They can:
  {can}.* *They see: {libraries}.*
* When the account holds permissions outside the friend set: an amber `Alert` *{name} also has: {extras}.
  These aren't for the internet.* and the primary button becomes **Remove them and let in** (the client
  reads the user with the existing user query and calls the existing `updateUser` with the permissions
  minus the extras, then `setPersonAccess(publicSignin: true)`); secondary *Edit account*.
* *Libraries*: radio *Leave as they are* / *Only Biblioteca* (as today).
* Checkbox *Set a new password now (recommended for accounts made before)* → opens `SetPasswordDialog`
  after saving.

### 2.7 History (collapsed)

`NewCard` *History*, collapsed to one ghost button *Show history*. Expanded: `Tabs` *Sign-ins* |
*Changes*, 20 rows a page with the existing `Pager`.

* **Sign-ins**: other people only (the owner's successful sign-ins are left out; failures on the owner's
  name stay, §3.4). Row: name, result `Badge` (*Signed in*, *Wrong password*, *Device limit*, *Not
  allowed from the internet*, *Locked*, *Owner, refused from the internet*), second line *{when} · {device label or
  agent label}*; tap expands IP and the full agent. Toggle *Failures only*. Footer link *Every sign-in,
  also at home, is in Users → Authentication history.*
* **Changes**: the audit rows as translated sentences from `action` + the target's name (reuse the
  existing `activity.actions.*` labels, which are already in en and es), e.g. *Device removed: Chrome on
  Linux*, *Link made: Club de lectura*, *Marked as seen: 3 items*; second line *{who} ·
  {when}* where *public* reads *From the internet*. The server's `detail` text is shown only when the row
  is expanded. No UUIDs on screen.

### 2.8 Status line and Details

* Ready: `CheckCircle2` green + *Open to the internet at {host}.* + *{n} people can sign in.*;
  right-aligned ghost *Details*.
* Ready, public sign-in off (environment): *Open at {host} with device links only.*
* Global budget tripped: red dot + *Letting nobody sign in for a few minutes.* (also an `ALERT` item).
* Not configured: an `Alert` (warning) replaces the line: title *Public access isn't set up*, text *The
  server is missing {missing}. The deploy notes explain how to set it.* (`the key`, `the public
  address`, `the state folder`, joined; es *Al servidor le falta {missing}. Las notas de despliegue
  explican cómo configurarlo.*). The People section stays visible; *Invite someone* is disabled with a
  `ToolTip` *Set up public access first.* / *Primero configura el acceso público.*
* Public sign-in off on the server (`publicSignin` false): signup links can't be made (the server
  refuses them today). *Invite someone* stays, but its dialog says *New accounts need public sign-in,
  which is off on the server. You can still add a device for someone who has an account: open their
  name.* / *Las cuentas nuevas necesitan la entrada pública, que está apagada en el servidor. Aún puedes
  añadir un dispositivo a quien ya tiene cuenta: abre su nombre.* and shows no form.
* **Details** dialog (`Dialog`, `NewCard.Row`s): *Public address*, *Devices need a pass* (Yes/No), *Public
  sign-in* (*On, set on the server* / *Off, set on the server*), *Edge updated* ({when}), *Failed sign-ins
  in the last 10 min*, *Names slowed down now*, and the note *The edge also slows down a client that tries
  too often; it clears by itself within a minute.*

### 2.9 Wording (en-US → es-ES), new keys under `settingsScene.server/access`

| Key | en-US | es-ES |
|-----|-------|-------|
| `status.open` | Open to the internet at {{host}}. | Abierto a internet en {{host}}. |
| `status.people_one` / `_other` | {{count}} person can sign in. / {{count}} people can sign in. | {{count}} persona puede entrar. / {{count}} personas pueden entrar. |
| `status.linksOnly` | Open at {{host}} with device links only. | Abierto en {{host}} solo con enlaces de dispositivo. |
| `status.refusing` | Letting nobody sign in for a few minutes. | No deja entrar a nadie durante unos minutos. |
| `status.details` | Details | Detalles |
| `attention.title` | Needs your attention ({{count}}) | Necesita tu atención ({{count}}) |
| `attention.clearAll` | Clear all | Descartar todo |
| `attention.dismiss` | Dismiss | Descartar |
| `attention.dismissed` | Marked as seen. It comes back if something changes. | Marcado como visto. Volverá si algo cambia. |
| `attention.cleared_other` | {{count}} marked as seen. History keeps them. | {{count}} marcados como vistos. El historial los guarda. |
| `attention.undo` | Undo | Deshacer |
| `attention.allGood` | All good. Nothing needs your attention. | Todo bien. Nada necesita tu atención. |
| `attention.showAll` | Show all ({{count}}) | Ver todo ({{count}}) |
| `people.title` | People | Personas |
| `people.description` | Who can sign in from the internet. | Quién puede entrar desde internet. |
| `people.invite` | Invite someone | Invitar a alguien |
| `people.groups.internet` | Can sign in from the internet ({{count}}) | Pueden entrar desde internet ({{count}}) |
| `people.groups.invited` | Invited, not in yet ({{count}}) | Invitados, aún sin entrar ({{count}}) |
| `people.groups.home` | Can't sign in from the internet ({{count}}) | No pueden entrar desde internet ({{count}}) |
| `people.showEnded` / `hideEnded` | Show ended ({{count}}) / Hide ended | Ver terminados ({{count}}) / Ocultar terminados |
| `people.internetBadge` | Internet | Internet |
| `people.devicesLine` | {{active}} of {{limit}} devices · last in {{when}} | {{active}} de {{limit}} dispositivos · última vez {{when}} |
| `people.never` | never signed in | nunca ha entrado |
| `people.empty` | Nobody can sign in from the internet yet. | Aún nadie puede entrar desde internet. |
| `people.invitedOne` | One person · expires {{when}} | Una persona · caduca {{when}} |
| `people.invitedOpen` | Several people · {{used}} of {{max}} joined · expires {{when}} | Varias personas · {{used}} de {{max}} se unieron · caduca {{when}} |
| `people.invitedDevice` | Device link for {{name}} · expires {{when}} | Enlace de dispositivo para {{name}} · caduca {{when}} |
| `person.internet` | Sign in from the internet | Entrar desde internet |
| `person.internetHelp` | With their password, on up to {{limit}} devices. | Con su contraseña, en hasta {{limit}} dispositivos. |
| `person.devices` | Devices ({{active}} of {{limit}}) | Dispositivos ({{active}} de {{limit}}) |
| `person.used` | Used {{when}} | Usado {{when}} |
| `person.allowMoreTries` | Allow more tries | Permitir más intentos |
| `person.allowMoreTriesHelp` | After too many wrong passwords on this device. | Tras demasiadas contraseñas incorrectas en este dispositivo. |
| `person.removeDevice` | Remove | Quitar |
| `person.removeDeviceConfirm` | Remove {{label}}? It stops working within seconds. | ¿Quitar {{label}}? Deja de funcionar en segundos. |
| `person.limit` | Device limit | Límite de dispositivos |
| `person.addDeviceLink` | Add a device with a link | Añadir un dispositivo con un enlace |
| `person.showRemoved` | Show removed devices ({{count}}) | Ver dispositivos quitados ({{count}}) |
| `person.restore` | Restore | Restaurar |
| `person.setPassword` | Set a new password | Poner una contraseña nueva |
| `person.signOutAll` | Sign out everywhere | Cerrar sesiones |
| `person.signOutAllConfirm` | {{name}} will have to sign in again everywhere. | {{name}} tendrá que volver a entrar en todos sus dispositivos. |
| `person.locked` | Locked after {{count}} wrong passwords. | Bloqueado tras {{count}} contraseñas incorrectas. |
| `person.unblock` | Unblock | Desbloquear |
| `person.sees` | Sees: {{libraries}} | Ve: {{libraries}} |
| `person.hideNew` | Hide new libraries | Ocultar bibliotecas nuevas |
| `person.uploadLimits` | Upload limits | Límites de subida |
| `person.editAccount` | Edit account | Editar cuenta |
| `person.removeAll` | Remove all devices | Quitar todos los dispositivos |
| `person.removeAllConfirm` | All of {{name}}'s devices stop working within seconds. | Todos los dispositivos de {{name}} dejan de funcionar en segundos. |
| `invite.title` | Invite someone | Invitar a alguien |
| `invite.who` / `whoPlaceholder` / `whoHelp` | Who is it for? / e.g. Aunt Rosa / Only you see this. | ¿Para quién es? / p. ej. Tía Rosa / Solo tú lo ves. |
| `invite.whoMissing` | Write who it's for. | Escribe para quién es. |
| `invite.one` / `several` | One person / Several people | Una persona / Varias personas |
| `invite.howMany` | How many accounts | Cuántas cuentas |
| `invite.worksFor` | Works for | Vale por |
| `invite.create` | Create link | Crear enlace |
| `invite.sendTo` | Send this link to {{note}} | Envía este enlace a {{note}} |
| `invite.copy` / `share` / `showQr` | Copy / Share / Show QR code | Copiar / Compartir / Ver código QR |
| `invite.onlyNow` | You'll only see it now. If it gets lost, make a new one. | Solo lo verás ahora. Si se pierde, crea otro. |
| `invite.done` | Done | Listo |
| `letIn.title` | Let {{name}} in from the internet? | ¿Dejar entrar a {{name}} desde internet? |
| `letIn.body` | They'll sign in with their password on up to {{limit}} devices. | Entrará con su contraseña en hasta {{limit}} dispositivos. |
| `letIn.extras` | {{name}} also has: {{extras}}. These aren't for the internet. | {{name}} también tiene: {{extras}}. No son para internet. |
| `letIn.removeAndLetIn` | Remove them and let in | Quitarlos y dejar entrar |
| `letIn.can` / `letIn.sees` | They can: {{can}}. / They see: {{libraries}}. | Puede: {{can}}. / Ve: {{libraries}}. |
| `letIn.keepLibraries` / `onlyBiblioteca` | Leave as they are / Only Biblioteca | Dejarlas como están / Solo Biblioteca |
| `letIn.setPassword` | Set a new password now (recommended for accounts made before) | Poner una contraseña nueva ahora (recomendado para cuentas anteriores) |
| `letIn.turnOff` | {{name}} won't be able to come in from the internet on any device. | {{name}} no podrá entrar desde internet en ningún dispositivo. |
| `history.title` / `show` | History / Show history | Historial / Ver historial |
| `history.signIns` / `changes` | Sign-ins / Changes | Entradas / Cambios |
| `history.failuresOnly` | Failures only | Solo fallos |
| `history.allSignIns` | Every sign-in, also at home, is in Users → Authentication history. | Todas las entradas, también en casa, están en Usuarios → Historial de autenticación. |
| `history.fromInternet` | From the internet | Desde internet |
| `history.result.ok` / `wrongPassword` / `deviceLimit` / `notAllowed` / `locked` / `ownerViaEdge` | Signed in / Wrong password / Device limit / Not allowed from the internet / Locked / Owner, refused from the internet | Entró / Contraseña incorrecta / Límite de dispositivos / Sin permiso desde internet / Bloqueado / Dueño, rechazado desde internet |
| `activity.actions.warningDismiss` / `warningRestore` (camelCase like the existing `inviteCreate`) | Marked as seen / Shown again | Marcado como visto / Vuelto a mostrar |
| `person.lastIn` / `person.neverIn` | Last in {{when}} from {{device}} / Never signed in | Última vez {{when}} desde {{device}} / Nunca ha entrado |
| `person.kind.browser` / `readingApp` / `terminal` | Browser / Reading app / Terminal pass | Navegador / App de lectura / Pase del terminal |
| `attention.raised` | {{name}} can now use {{count}} devices. | {{name}} ya puede usar {{count}} dispositivos. |
| `attention.badge` | {{count}} things need your attention | {{count}} cosas necesitan tu atención |
| `details.title` | Public access details | Detalles del acceso público |
| `details.address` / `pass` / `signin` / `edge` / `failures` / `slowed` | Public address / Devices need a pass / Public sign-in / Edge updated / Failed sign-ins in the last 10 min / Names slowed down now | Dirección pública / Los dispositivos necesitan pase / Entrada pública / Edge actualizado / Intentos fallidos en los últimos 10 min / Nombres frenados ahora |
| `details.edgeNote` | The edge also slows down a client that tries too often; it clears by itself within a minute. | El edge también frena a quien lo intenta demasiado; se pasa solo en un minuto. |

Attention sentences: the table in §2.3 (keys `attention.kinds.<kind>` with `_one`/`_other`; action keys
`attention.actions.<action>`). Keys of removed parts (`status.*` strip labels, `invites.*` section texts,
`devices.*` section texts, `signIns.*` section texts, `warnings.*`) are deleted with their code; the
locale parity test must stay green.

### 2.10 Phone layout (390 px)

* Status line wraps to two lines; *Details* stays on the first line's right.
* *Invite someone* full width under the People description (still the only primary button).
* Attention items: sentence on line 1, action button + ✕ right-aligned on line 2.
* People rows: name + badges on line 1, the devices line on line 2, chevron right; no other controls.
* Person sheet = bottom `Drawer` (90 dvh, sticky title with a close button, content scrolls).
* Invite dialog = full-screen `Drawer`; *Share* shown first in the result step when available.
* History collapsed; rows two lines.

### 2.11 Components

`NewCard`, `NewCard.Row`, `Button` (`primary` once), `IconButton`, `DropdownMenu`, `Badge`, `Alert`,
`Sheet`, `Drawer`, `Dialog`, `ConfirmationModal`, `WideSwitch`, `NativeSelect`, `RadioGroup`, `Input`,
`Tabs`, `ToolTip`, `Avatar`, `Text`, `Heading`; icons from `lucide-react` (already used). Files: replace
`WarningsStrip.tsx` with `AttentionSection.tsx`, `StatusStrip.tsx` with `StatusLine.tsx` +
`DetailsDialog.tsx`, merge `PeopleSection.tsx` + `DeviceSection.tsx` + `InviteSection.tsx` into
`PeopleSection.tsx` + `PersonSheet.tsx` + `DeviceLinkForm.tsx`, turn `SignupLinkSection.tsx` into
`InviteDialog.tsx` (keep `FRIEND_PERMISSIONS`, the preset summary and editor), `SignInSection.tsx` +
`ActivitySection.tsx` into `HistorySection.tsx`. `AccessScene.tsx` renders: StatusLine, AttentionSection,
PeopleSection, HistorySection (`gap-y-8`).

### 2.12 Acceptance checks (Public access)

* **A1** 1440×900, no scrolling: the status line, *Needs your attention* (or *All good*), the People
  heading with *Invite someone*, and the first 3 people rows are visible.
* **A2** 390×844, no scrolling: the status line, the attention heading with its first item (or *All
  good*), and *Invite someone* are visible.
* **A3** With the sandbox data of the *before* shots (5 warnings, 4 accounts, 3 links, 9 devices, ~40
  sign-ins, ~25 audit rows), History collapsed: page ≤ 1,800 px tall on desktop and ≤ 3,400 px on a
  phone (today 7,014 and 10,506).
* **A4** Exactly one `primary` button outside dialogs: *Invite someone*.
* **A5** The owner isn't in People; no revoked device, revoked/expired/used-up link or invite, and no
  terminal leftover is visible until *Show ended* / *Show removed devices* (the owner's "Show revoked").
* **A6 (owner)** *Needs your attention* is the first section and gathers every kind of §2.3; each item
  is one line with exactly one action and a *Dismiss*; *Clear all* marks all of them seen; the count
  beside *Public access* and on the gear equals the number of items; after *Clear all* both show nothing
  without a reload; a new event of a dismissed kind brings the item and the badge back; History → Changes
  shows *Marked as seen*; no row of `user_login_activity` or `access_audit` is deleted.
* **A7** Nothing to show: the single *All good* line, no badge anywhere.
* **A8** No raw user agent, UUID or server-made English on the first view or in History rows (only in an
  expanded row's detail); IPs only in expanded rows.
* **A9** Every new string in en-US and es-ES (locale test), no raw key on screen.
* **A10** 390 px: no horizontal scroll; every control ≥ 40 px tall; the person sheet is a bottom drawer;
  dialogs trap focus and close with Esc.
* **A11** Invite flow: *Create link* → address shown once with *Copy*, *Share* where supported, QR only
  after *Show QR code*; the address is never in a list; closing the dialog forgets it.

## 3. Server changes for dismissing (one commit, one migration)

The warnings are computed on request (`crates/models/src/services/access_warnings.rs`, ids like
`AtDeviceLimit:<person id>`, stable across requests). Dismissing needs to remember *what the owner saw*.

### 3.1 Table and migration

Migration `crates/migrations/src/m20261116_000000_access_warning_reviews.rs`, registered after
`m20261115_000000_friend_onboarding` in `crates/migrations/src/lib.rs`:

```sql
CREATE TABLE access_warning_reviews (
  warning_id      TEXT    NOT NULL PRIMARY KEY,  -- Warning::id(), e.g. "AtDeviceLimit:<uuid>", "SigninFailures:-"
  dismissed_at    TEXT    NOT NULL,              -- timestamptz, as the other access tables
  dismissed_by    TEXT    NULL REFERENCES users(id) ON DELETE SET NULL,
  count_at_dismiss INTEGER NOT NULL,             -- Warning.count when dismissed
  last_at_dismiss TEXT    NOT NULL               -- Warning.last_at when dismissed
);
```

`down()` drops the table. Manual down `crates/migrations/manual-down/m20261116_000000_access_warning_reviews.sql`
(copy in `research/2026-11-16-access-warning-reviews-manual-down.sql`, same header style as the friend
onboarding one): `BEGIN; DROP TABLE IF EXISTS access_warning_reviews; DELETE FROM seaql_migrations
WHERE version = 'm20261116_000000_access_warning_reviews'; COMMIT;` and the check query. What is lost: only
which warnings were dismissed (they show again). The migration test runs the manual-down file like
`the_manual_down_leaves_a_database_the_previous_image_can_use` does. Snapshot the live db before deploying
(AGENTS.md step 4). Entity: `crates/models/src/entity/access_warning_review.rs`.

### 3.2 Warning computation (`access_warnings.rs`)

* `Warning` gains `last_at: DateTime<Utc>`: the time of the newest fact behind it. Event kinds
  (`DeviceLimitHit`, `NameBackoff`, `GlobalBudget`, `SigninFailures`, `NotEnabledTried`): the newest
  event (track it in the `and_modify` of the grouping). State kinds: `AtDeviceLimit`/`OverDeviceLimit`/
  `ManyDevices` = newest active device `created_at` (today's `since`); `ManySessions` = newest session
  `created_at`; `NotFriendClass` = the person row's `created_at`; links = the link's newest use or
  `created_at`; new kinds below.
* `NotFriendClass.count` = the number of extra permissions (so a new one counts as "worse").
* Event warnings whose person no longer exists (or is deleted) are **dropped** (fixes "was refused a new
  device 1 times…" with no name).
* `NameBackoff` without a known account is **merged** into the no-person `SigninFailures` (one item,
  `detail = "slowed"` when any backoff happened). `GlobalBudget` stays separate (alert).
* New kinds (`WarningKind` + GraphQL `AccessWarningKind`):
  * `AccountLocked` (person; count = failures in 24 h; last_at = last failure) from
    `service::blocked_users`.
  * `LeftoverDevices` (no person; count = active devices that are legacy terminal passes, or whose
    person is missing/removed, or unused for `DORMANT_AFTER_DAYS` (60); last_at = newest of their
    `created_at`/`last_seen_at`).
  * `BooksWaiting` (no person; count = intake items in `PENDING_APPROVAL`; last_at = newest submission).
    For the owner it replaces the separate approvals badge on the gear (§4).
* Visibility: a warning is **dismissed** iff a review row with its id exists and
  `warning.count <= count_at_dismiss` and `warning.last_at <= last_at_dismiss`. So a new event or a worse
  state shows it again; an event fading out of the 3-day window (count going down) does not.
* `warning_count()` counts the visible ones only (the badge = the list).
* `dismiss(conn, now, by, ids: Option<Vec<String>>) -> usize`: upserts a row for each id among the
  *currently visible* warnings (all of them when `ids` is `None`), unknown ids ignored; deletes rows older
  than 30 days whose warning no longer exists. `restore(conn, ids) -> usize` deletes the rows.

### 3.3 GraphQL

```graphql
type AccessWarning { …existing fields…  lastAt: DateTime!  dismissedAt: DateTime }
enum AccessWarningKind { …existing…  ACCOUNT_LOCKED  LEFTOVER_DEVICES  BOOKS_WAITING }

type Query {
  accessWarnings(includeDismissed: Boolean! = false): [AccessWarning!]!   # default: visible only
  accessWarningCount: Int!                                                 # visible only
}
type Mutation {
  "Marks warnings as seen. Without ids: every warning shown now (Clear all). Returns how many"
  dismissAccessWarnings(ids: [ID!]): Int!
  "Shows dismissed warnings again (Undo). Returns how many"
  restoreAccessWarnings(ids: [ID!]!): Int!
}
```

* Guards: queries keep `ServerOwnerGuard.and(NotViaPublicEdgeGuard)`; both mutations
  `ServerOwnerGuard.and(NotViaPublicEdgeGuard).and(SameOriginGuard)` (as every access mutation).
* Audit (`service::audit`): one row per call, action `warning.dismiss` (target = the single id or
  `all`, detail = `{n} items`) and `warning.restore`, in the style of `invite.create`/`device.revoke`.
  History → Changes labels them *Marked as seen* /
  *Shown again* (en) and *Marcado como visto* / *Vuelto a mostrar* (es).

### 3.4 Small additive query arguments (no migration)

* `accessDevices(personId: ID, includeEnded: Boolean! = true, pagination)`: `includeEnded: false` leaves
  out revoked devices; `personId` filters one person (the sheet).
* `accessInvites(includeEnded: Boolean! = true, …)` and `accessSignupLinks(includeEnded: Boolean! =
  true, …)`: `false` = only open ones (not revoked, not expired, uses left).
* `accessSignIns(othersOnly: Boolean! = false, …)`: `true` leaves out the server owner's *successful*
  sign-ins (failures on the owner's name stay).
* `AccessSignIn.agentLabel: String`: `access_policy::device_label(user_agent)` ("Chrome on Linux").
* `AccessAuditEntry.targetLabel: String` (the type `accessAudit` returns): the name behind `target`
  when it is the id of a device (its label), a signup link (its note), an invite (its label) or an
  account (its username); `null` otherwise. History → Changes shows it instead of the id.

Defaults keep today's behaviour, so the old web code and tests keep working until replaced.

### 3.5 Server tests

Dismiss hides from list and count; restore shows again; a new `signin.device_limit` audit row after a
dismissal shows `DeviceLimitHit` again; a new device for a person at the limit shows `AtDeviceLimit`
again; an event fading (count down) stays hidden; unknown ids are ignored; a request through the edge and
a non-owner are refused (both mutations); a warning of a removed person is not listed; the unknown-name
merge yields one item; `BooksWaiting`, `AccountLocked`, `LeftoverDevices` appear and count; the manual
down leaves a database the previous image can use.

## 4. Sidebar badge and settings navigation

Problems: two badges stacked on the 16 px gear, the access one at `-bottom` and clipped by the window
(`owner-home-sidebar-desktop.png`), overlapping text in the phone sidebar (`owner-sidebar-open-phone.png`);
no count in the phone settings menu (`owner-settings-menu-phone.png`); nothing on the phone header.

New:

* **Owner**: one badge on the gear = `accessWarningCount` (which includes `BOOKS_WAITING`), top-right
  (`-right-1 -top-1`, the `ApprovalsBadge` position), amber, `99+` cap; `sr-only` and `ToolTip`: *{n}
  things need your attention* / *{n} cosas necesitan tu atención*. `ApprovalsBadge` is not rendered for the
  owner.
* **Other approvers** (Manage library, not owner): `ApprovalsBadge` as today (they can't see Public
  access).
* **Settings navigation** (desktop `SettingsSideBar` and the phone `BreadcrumbPageSwitcher` drawer):
  inline counts beside *Public access* (attention count) and *Book approvals* (books waiting).
* **Phone header**: a small amber dot on the sidebar toggle button when the owner's count > 0
  (`aria-label` gets *, {n} need your attention*).
* *Clear all* and *Dismiss* invalidate `PublicAccessWarningCount`; polling stays at 60 s.
* Fix the nav label: add `settingsScene.sidebar.management["book approvals"]` = *Book approvals* /
  *Aprobar libros* (today the menu shows `webApp.settings…`), and the missing es-ES *deleted books*
  (*Libros borrados*) and *recently added* (*Añadidos recientemente*).

Acceptance: **B1** at 1440×900 the gear badge is fully inside the window; **B2** one badge only for the
owner; **B3** on a phone the header dot and the menu counts show; **B4** after *Clear all* gear, nav and
dot show nothing; **B5** no raw key in any settings menu (en and es).

## 5. Owner: Settings → Book approvals (+ upload limits)

Problems (`owner-book-approvals-desktop.png`, `-phone.png`): English only (no locale keys in
`BookApprovalsScene.tsx`, `ReviewCard.tsx`, `UserLimits.tsx`, nor in the intake components it reuses);
the nav label is a raw key; metadata-source failures are four warning lines on every card ("Open Library:
unreachable", …; in the sandbox they are offline, but the same lines show whenever a source is down); the
*Upload limits* block is always open at the bottom with an arbitrary person preselected (*dani*).

New:

* Header line: *{n} books waiting. They join the library when you approve them.* / *{n} libros esperan.
  Entran en la biblioteca cuando los apruebas.* + the library filter (*All libraries* / *Todas las
  bibliotecas*). With 2+ ready: *Approve all ready ({n})* / *Aprobar todos los listos ({n})* (outline).
* Cards stay (they are the review form), with the source failures folded into one muted line *Couldn't
  check online ({n} sources).* / *No se pudo consultar en línea ({n} fuentes).* (expand to list them).
  Actions: *Approve* (primary) / *Aprobar*; *Send back…* / *Devolver…*; overflow *Discard* / *Descartar*.
* *Upload limits* becomes a collapsed `Accordion` item *Upload limits per person* / *Límites de subida por
  persona* (anchor `#limits`, opened by `?person={id}`); no person preselected (*Choose a person* /
  *Elige una persona*); labels *Biggest file* / *Archivo más grande*, *Files per upload* / *Archivos por
  subida*, *Inbox space* / *Espacio en la bandeja*, *Books at once* / *Libros a la vez*; help *Leave a box
  empty to use the server's limit.* / *Deja una casilla vacía para usar el límite del servidor.*; *Save* /
  *Guardar*, *Use the server's limits* / *Usar los del servidor*.
* Empty: *Nothing is waiting for approval.* / *No hay libros esperando aprobación.*
* Keys under `settingsScene.server/bookApprovals.*` (exists with `title`, `description`).

Acceptance: **BA1** every string in en/es, no raw key; **BA2** source failures are one line; **BA3** the
limits block is closed by default and opens from the person sheet link with that person chosen.

## 6. Owner: user editor, permission bits (`/settings/users/:id/manage`)

Problems (`owner-user-editor-*.png`): es-ES has no labels for *Add books* and *Remove books* (the
`ADD_BOOKS` and `REMOVE_BOOK` entries are missing, English shows); the en description of *Add books*
("upload books and add them to libraries") is wrong for friends, whose books wait for approval; nothing in
the editor says which permissions keep a person off the internet (the Public access page calls them
"beyond the preset").

New:

* `ADD_BOOKS`: en *Add Books* — *Allows the user to upload books and send them for approval (people who
  manage libraries add them directly)*; es *Añadir libros* — *Permite subir libros y enviarlos para
  aprobar (quien gestiona bibliotecas los añade directamente)*.
* `REMOVE_BOOK` es: *Quitar libros* — *Permite quitar libros de las bibliotecas*.
* When the person has public sign-in on (from `accessPeople`), each checked permission outside
  `FRIEND_PERMISSIONS` shows a small amber `Badge` *Not for the internet* / *No es para internet*, and one
  line above the table: *{name}'s internet sign-in is on. A permission marked “Not for the internet”
  keeps them out.* / *{name} tiene activada la entrada desde internet. Un permiso marcado «No es para
  internet» se lo impide.*
* Unchanged: *Upload files* ticks nothing else (already so).

Acceptance: **U1** es labels show; **U2** the badge and line show for lucia with an extra permission and
disappear when it is removed.

## 7. Friend: Settings → My devices (`/settings/devices`)

Problems (`friend-my-devices-*.png`): the count is said twice ("You use 3 of 3 devices." and the banner
"You use all 3 of your devices…"); IP addresses ("from 127.0.0.1", "from –") mean nothing to a friend; a
pencil icon without a label; a long footer ("an iphone's home screen").

New:

* One line: *You're using {active} of {limit} devices.* / *Usas {active} de {limit} dispositivos.* At the
  limit the line becomes an amber `Alert`: *You're using all {limit} devices. To use a new one, remove one
  here.* / *Usas tus {limit} dispositivos. Para usar otro, quita uno aquí.*
* A `NewCard` list (not a card grid): kind icon, name, `Badge` *This device* / *Este dispositivo*, kind
  `Badge` *Browser* / *Navegador*, *Reading app* / *App de lectura*; second line *Used {when}* / *Usado
  {when}*. Overflow: *Rename* / *Cambiar nombre*, *Remove* / *Quitar* (this device: *Remove and sign out*
  / *Quitar y cerrar sesión*), with the existing confirmations. No IP.
* Footer (muted): *Reading apps count as devices. Adding Stump to an iPhone home screen counts as one
  more.* / *Las apps de lectura cuentan como dispositivos. Añadir Stump a la pantalla de inicio del iPhone
  cuenta como uno más.*
* Empty: *No devices yet. Sign in from a phone, tablet or computer and it shows up here.* / *Aún no hay
  dispositivos. Entra desde un teléfono, tableta o computadora y aparecerá aquí.*

Acceptance: **F1** the count appears once; **F2** no IP on the page; **F3** each row has one overflow
menu with rename and remove; **F4** 390 px: rows full width, no horizontal scroll.

## 8. Friend: Settings → Reading apps (`/settings/reading-apps`)

Problems (`friend-reading-apps-*.png`): jargon and lowercase names in the intro ("Connect koreader, a kobo
or any reading app that supports OPDS… the key is part of it"); the add form is always shown, and at the
device limit its button is disabled but still orange; the three-step instructions repeat in every card;
"Replace address" doesn't say the old one stops working.

New:

* Intro: *Read your books in KOReader, Kobo or another reading app.* / *Lee tus libros en KOReader, Kobo u
  otra app de lectura.*
* One primary button *Add a reading app* / *Añadir una app de lectura* → `Dialog`: *Name* / *Nombre*
  (placeholder *My Kobo* / *Mi Kobo*) → *Add* / *Añadir* → result step: *Your address for {name}* / *Tu
  dirección para {name}*, the address with *Copy*, *Show QR code*, and the steps: 1. *In the app, add an
  OPDS catalog (KOReader: Search → OPDS catalog → +).* / *En la app, añade un catálogo OPDS (KOReader:
  Buscar → Catálogo OPDS → +).* 2. *Paste this address. Leave user and password empty.* / *Pega esta
  dirección. Deja usuario y contraseña vacíos.* 3. *Open the catalog and download your books.* / *Abre el
  catálogo y descarga tus libros.* Note: *Keep this address to yourself: it opens your books.* / *No
  compartas esta dirección: abre tus libros.*
* List rows: name, *Used {when}* / *Never used* (*Usado {when}* / *Sin usar*); *Show address* (outline,
  opens the same result step); overflow *New address* (*The old one stops working.*) / *Nueva dirección*
  (*La anterior deja de funcionar.*), *Remove* / *Quitar*.
* At the limit: the button disabled with the line *You're using all {limit} devices. Remove one in My
  devices to add an app.* / *Usas tus {limit} dispositivos. Quita uno en Mis dispositivos para añadir una
  app.* (link to My devices). The existing "no Download permission / edge not set" reasons stay.
* Empty: *No reading apps yet.* / *Aún no hay apps de lectura.*

Acceptance: **F5** no form on the first view, one primary button; **F6** address and QR only after *Add*
or *Show address*; **F7** brand names capitalised in en and es.

## 9. Friend: Add books and the intake list (`/add-books`)

Problems (`friend-add-books-*.png`, `owner-add-books-*.png`): no translations at all (every string is
hard-coded English in `scenes/intake/*`, and the sidebar label *Add books* too); each book in *Needs your
details* is the full edit form (title, authors, series, a dozen genre chips such as "Level: General
Reader", description), so 4 books are 2,832 px on desktop and 4,798 px on a phone; a book that is the same
file as one in the library still offers **Send anyway**, which the server refuses ("'Libro_de_horas.epub'
is already in the library"); source failures ("Open Library: unreachable") are shown to friends; the quota
("Your inbox: 548 KB of 2.0 GB", "4 of 100 books") sits on the first line.

New:

* Translate everything: keys `intakeScene.*` (en/es) and `sidebar.buttons.addBooks` = *Add books* /
  *Añadir libros*.
* Intro: *Send books to the library. They're added once approved.* / *Envía libros a la biblioteca. Se
  añaden cuando se aprueban.* (approvers keep their own text).
* Groups with counts: *Check these ({n})* / *Revisa estos ({n})*; *Ready to send ({n})* / *Listos para
  enviar ({n})* with *Send all* / *Enviar todos*; *Waiting for approval ({n})* / *Esperando aprobación
  ({n})*; *Sent back ({n})* / *Devueltos ({n})* with the approver's note; *Added ({n})* / *Añadidos
  ({n})*.
* Compact rows (cover, title, author, one status line) with one action: *Send* / *Enviar*, *Check* /
  *Revisar* (expands the existing form in place, one at a time), *Take back* / *Retirar*. Genres: only the
  chosen ones plus *Suggestions ({n})* / *Sugerencias ({n})* collapsed.
* Same file already in the library: the line *Already in the library.* / *Ya está en la biblioteca.* and
  the action *Remove from my list* / *Quitar de mi lista*; no *Send anyway*. (A similar title that is not
  the same file keeps *Send anyway* / *Enviar igualmente* as a secondary button.)
* Source failures are not shown to people who need approval.
* Quota only from 80 %: *Your inbox is almost full ({used} of {total}).* / *Tu bandeja está casi llena
  ({used} de {total}).*
* Drop zone texts: *Drop books here* / *Suelta aquí tus libros*; *or choose files: EPUB, PDF, CBZ or CBR;
  folders work too.* / *o elige archivos: EPUB, PDF, CBZ o CBR; también carpetas.*; *Choose files* /
  *Elegir archivos*; *Choose a folder* / *Elegir carpeta*.

Acceptance: **F8** every string in en/es (es-ES locale shows no English); **F9** a same-file duplicate
has no *Send anyway*; **F10** with the 4 sandbox books, the phone page is ≤ 2,000 px with rows collapsed.

## 10. Order of work (commit after each; Conventional Commits + gitmoji)

1. `fix(web)`: Book approvals nav label + missing es-ES nav keys (tiny, ships alone).
2. `feat(access)`: server — `access_warning_reviews` (migration, manual down, entity), `last_at`, merge
   and removed-person rules, new kinds, `dismissAccessWarnings` / `restoreAccessWarnings`, the additive
   query arguments, `agentLabel`, `targetLabel`, tests; regenerate `schema.graphql` and the client (`scripts/fork/
   schema.sh` is docker-based: run it on g3, its comment still says l5). Cold cache: g3 only
   (`G3_CARGO_MEM=7g G3_CARGO_JOBS=1 g3-cargo …`).
3. `feat(web)`: Public access — StatusLine + Details, AttentionSection, People + PersonSheet +
   DeviceLinkForm, InviteDialog, History; delete the replaced sections and their keys; tests (vitest) for
   the attention list (dismiss, clear all, undo, empty), people grouping, the sheet, the invite flow.
4. `feat(web)`: badges (one gear badge, phone dot, menu counts).
5. `feat(web)`: My devices and Reading apps.
6. `feat(web)`: Add books and Book approvals translations, duplicate action, compact rows, collapsed
   limits.
7. `fix(web)`: user editor bits.

## 11. Taking the *after* screenshots

The *before* set was made with image `stump:fork-71310fb5a` (no image of `43fa3fe50` exists; the commits
in between touch only the public sign-in pages, the edge docs and one error message, none of these
screens). Sandbox: a `sqlite3 -readonly … .backup` copy of the live db in `/tmp/uxbox/config` with a known
password for the owner, an internal network (`docker network create --internal uxnet`), the library
folders mounted read-only at the compose paths, `STUMP_EDGE_*` env with a random key and
`STUMP_EDGE_PUBLIC_URL=https://books.example.org`, `STUMP_INTAKE_PROBE_MODE=subprocess`. Data made
through the sandbox's API: a one-person signup link used by `lucia` (POST `/api/_i/join` with
`X-Edge-Public: 1` and `Origin` = the public URL), a second browser device for her (POST
`/api/_i/signin`), 4 wrong passwords for `dani` and one for an unknown name through the same header, an
open link *Club de lectura* (5 accounts), a device link for dani, a reading app *Kobo Clara*, and four
books in lucia's inbox (two library duplicates, two generated EPUBs, one sent for approval). The friend's
browser is "through the edge" by sending `X-Edge-Public: 1` and `X-Edge-Pass: <pass from the join>` on
every request (Playwright `extraHTTPHeaders`). The app scrolls inside its main column, so full-page shots
grow the window to that column's `scrollHeight` first. Playwright `v1.62.1-noble` (the repo's version).

The image serves the web from `STUMP_CLIENT_DIR=/app/client`. For a web-only change: `bash
scripts/fork/node.sh 'yarn web build'` and add `-v "$PWD/apps/web/dist:/app/client:ro"` to the sandbox's
`docker run`. A server change (§3) needs an image built from this branch on g3. Save as
`research/ux-2026-11-12/after/<same names>.png` so before/after pair up.

## Appendix: the *before* screenshots (`research/ux-2026-11-12/before/`)

| File | Shows |
|------|-------|
| `owner-access-desktop.png`, `owner-access-phone.png` | Public access, whole page (1440×7014; 390×10506 at 1×) |
| `owner-access-first-view-desktop.png`, `owner-access-first-view-phone.png` | What fits on the first screen |
| `owner-access-preset-editor-desktop.png`, `-phone.png` | Signup links with *Change…* open |
| `owner-access-link-created-desktop.png`, `-phone.png` | After *Create link* (address and QR masked) |
| `owner-access-set-password-dialog-desktop.png`, `-phone.png` | Set a password (generated password masked) |
| `owner-access-public-signin-dialog-desktop.png`, `-phone.png` | Turning on public sign-in for an account with extra permissions |
| `owner-book-approvals-desktop.png`, `-phone.png` | Book approvals with one waiting book, Upload limits, the raw nav key |
| `owner-user-editor-desktop.png`, `-phone.png`, `owner-user-editor-permissions-*.png` | User editor; permissions scrolled to *Add books* |
| `owner-users-desktop.png`, `-phone.png` | Users page: its Authentication history duplicates Recent sign-ins |
| `owner-home-sidebar-desktop.png`, `-phone.png` | Gear badge clipped (desktop); nothing visible on a phone |
| `owner-sidebar-open-phone.png`, `owner-settings-menu-phone.png` | Two stacked badges; phone settings menu without counts |
| `owner-add-books-desktop.png`, `-phone.png` | The owner's Add books |
| `friend-my-devices-desktop.png`, `-phone.png` | My devices at the limit, through the edge (*This device*) |
| `friend-reading-apps-desktop.png`, `-phone.png` | Reading apps at the limit (address masked) |
| `friend-add-books-desktop.png`, `-phone.png` | Add books with 3 books to check and 1 waiting |
| `friend-settings-menu-phone.png` | A friend's settings menu on a phone |
