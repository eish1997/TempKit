'use strict';
// Suppress WebView browser menus, but keep dispatching to QuickGen's own handlers.
// Do not intercept keydown, pointer, clipboard or drag events.
window.addEventListener('contextmenu',event=>event.preventDefault(),{capture:true});
