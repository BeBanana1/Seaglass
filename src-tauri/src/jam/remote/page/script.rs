// The remote control page's `remote.js`, assembled at compile time from one file per area
// (#45), in this order: they are classic scripts sharing one scope, as when they were a
// single file. `include!`d by `remote/mod.rs`. Not a module: an expression only.
concat!(
    include_str!("text.js"),
    "\n",
    include_str!("core.js"),
    "\n",
    include_str!("live.js"),
    "\n",
    include_str!("browse.js"),
    "\n",
    include_str!("main.js"),
)
