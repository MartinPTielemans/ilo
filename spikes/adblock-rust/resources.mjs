// uBlock Origin's scriptlets and redirect stand-ins, from its source at the release ilo ships, as
// adblock-rust's resources: node resources.mjs UBO_SOURCE OUT.json. The scriptlets are functions,
// which adblock-rust calls with the filter's arguments; the stand-ins are the files in
// web_accessible_resources that redirect-resources.js names.

import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const [source, out] = process.argv.slice(2);
const js = path.join(source, 'src/js');
const { builtinScriptlets } = await import(pathToFileURL(path.join(js, 'resources/scriptlets.js')));
const redirects = (await import(pathToFileURL(path.join(js, 'redirect-resources.js')))).default;

const base64 = text => Buffer.from(text).toString('base64');
const resources = [];

for ( const scriptlet of builtinScriptlets ) {
    // A helper other scriptlets call, named *.fn, is fn/javascript; the rest are what +js() names.
    const helper = scriptlet.name.endsWith('.fn');
    let fn = scriptlet.fn.toString();
    // adblock-rust calls a scriptlet by the name its function has, as uBO does.
    if ( !fn.startsWith('function') ) {
        fn = `function ${scriptlet.fn.name}${fn.slice(fn.indexOf('('))}`;
    }
    resources.push({
        name: scriptlet.name,
        aliases: scriptlet.aliases ?? [],
        kind: { mime: helper ? 'fn/javascript' : 'application/javascript' },
        content: base64(fn),
        dependencies: scriptlet.dependencies ?? [],
        // Trusted scriptlets run only from uBO's own lists: main.rs gives those the first bit.
        ...(scriptlet.requiresTrust ? { permission: 1 } : {}),
    });
}

const mimes = { gif: 'image/gif', png: 'image/png', js: 'application/javascript', html: 'text/html',
    txt: 'text/plain', json: 'application/json', mp3: 'audio/mp3', mp4: 'video/mp4', xml: 'text/xml',
    css: 'text/css' };
let missing = 0;
for ( const [name, details] of redirects ) {
    const file = path.join(source, 'src/web_accessible_resources', name);
    if ( !fs.existsSync(file) ) { missing++; continue; }
    const alias = details.alias;
    resources.push({
        name,
        aliases: alias === undefined ? [] : Array.isArray(alias) ? alias : [alias],
        kind: { mime: mimes[path.extname(name).slice(1)] ?? 'application/octet-stream' },
        content: fs.readFileSync(file).toString('base64'),
    });
}

fs.writeFileSync(out, JSON.stringify(resources));
console.log(JSON.stringify({
    scriptlets: builtinScriptlets.length, redirects: redirects.size - missing, missing_redirect_files: missing,
}));
