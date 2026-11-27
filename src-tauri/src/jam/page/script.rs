// The guest page's `jam.js`, assembled at compile time from one file per area (#45), in this
// order: they are classic scripts sharing one scope, as when they were a single file.
// `include!`d by `jam/server.rs` (local Jam) and by the relay's `guest.rs` (online Jam), so
// both serve the same script. Not a module: an expression only.
concat!(
    include_str!("text.js"),
    "\n",
    include_str!("core.js"),
    "\n",
    include_str!("live.js"),
    "\n",
    include_str!("lyrics.js"),
    "\n",
    include_str!("search.js"),
    "\n",
    include_str!("game.js"),
    "\n",
    include_str!("main.js"),
)
