import {build} from 'esbuild';
import fs from 'node:fs';
import {fileURLToPath} from 'node:url';
const root=new URL('../',import.meta.url);
const icons={};for(const name of ['bold','highlighter','list-todo','quote','hash','remove-formatting','palette','pencil'])icons[name]=fs.readFileSync(new URL(`node_modules/lucide-static/icons/${name}.svg`,root),'utf8');
fs.writeFileSync(new URL('ui/note-icons.js',root),'// Official Lucide 0.468.0. See vendor/licenses/lucide-static.txt.\nObject.assign(window.quickgenIcons,'+JSON.stringify(icons)+');\n');
fs.copyFileSync(new URL('node_modules/lucide-static/LICENSE',root),new URL('ui/vendor/licenses/lucide-static.txt',root));
await build({entryPoints:[fileURLToPath(new URL('editor/editor.mjs',root))],outfile:fileURLToPath(new URL('ui/note-editor.bundle.js',root)),bundle:true,format:'iife',target:'es2022',minify:true,legalComments:'eof'});
