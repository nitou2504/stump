# Empty genre chip on the book page, 2026-11-12

Sandbox: a read-only `.backup` of the live database, image `stump:fork-a1a4db090` (the live build), no network, books
mounted read-only. `before/` is the live build; `after/` is the same server with this branch's web build mounted over
the served files (so it shows the web fix; the server fix is covered by the Rust tests).

* `before/before-bad-epub.png`, `before-phone-bad-epub.png`: *Annals of the Former World* (one of the 71): an empty chip
  starts the genre row (on a phone it is a blank round pill). It links to `genres hasAnyOf [""]`.
* `before/before-writers-bad.png`: the one book whose writers are `, Charlotte Brontë`: an empty author link.
* `before/before-empty-chip-click-friend.png`: what the empty chip's link opens for a friend: the Explore list filtered on
  nothing (71 unrelated books). No error page was reproduced, for the owner or a friend, desktop or phone, in-app click
  or full page load, with the filter and ordering panels opened.
* `after/`: the same books, no empty chip.
