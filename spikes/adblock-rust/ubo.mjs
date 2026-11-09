// uBlock Origin's network filtering engine, from its own source at the release ilo ships, on the
// same lists and requests as the Rust side, measured the same way: node ubo.mjs UBO STEP ARGS,
// where UBO is the folder uBO's tools/make-nodejs.sh made. Each step runs in a process of its own.
// It's uBO's network engine alone: its cosmetic, scriptlet and redirect engines don't run in Node.

import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const [ubo, step, ...args] = process.argv.slice(2);
const { StaticNetFilteringEngine, pslInit } = await import(pathToFileURL(path.join(ubo, 'index.js')));

const MB = 1024 * 1024;
const memory = () => {
    global.gc?.();
    const m = process.memoryUsage();
    return { resident_mb: m.rss / MB, heap_mb: m.heapUsed / MB, external_mb: (m.external + m.arrayBuffers) / MB };
};
// Node reports its peak resident set in KB on every OS (the peak working set on Windows).
const peak = () => ({ resident_mb: process.resourceUsage().maxRSS / 1024 });

const lists = dir => fs.readdirSync(dir).filter(f => f.endsWith('.txt')).sort()
    .map(f => ({ name: f.slice(0, -4), raw: fs.readFileSync(path.join(dir, f), 'utf8') }));

// The request's type as uBO names it, from the corpus's, as main.rs maps them: a document as a
// frame, as the corpus can't tell them apart.
const requestType = cpt => {
    switch ( cpt ) {
    case 'document': return 'sub_frame';
    case 'xhr': case 'fetch': return 'xmlhttprequest';
    case 'image': case 'script': case 'stylesheet': case 'font': case 'media': case 'websocket':
        return cpt;
    case 'ping': case 'beacon': return 'ping';
    default: return 'other';
    }
};

// The public suffix list from the list itself: the snapshot of it make-nodejs.sh's build.js writes
// reads back empty, and with no public suffixes every request is a third party's, as $3p filters
// then say.
const snfe = await StaticNetFilteringEngine.create({ noPSL: true });
pslInit(fs.readFileSync(
    path.join(ubo, 'assets/thirdparties/publicsuffix.org/list/effective_tld_names.dat'), 'utf8'));

if ( step === 'compile' ) {
    const [dir, out] = args;
    const read = lists(dir);
    const before = memory();
    let start = performance.now();
    await snfe.useLists(read);
    const compiled = performance.now() - start;
    const held = memory();
    start = performance.now();
    const selfie = await snfe.serialize();
    const serialized = performance.now() - start;
    fs.writeFileSync(out, selfie);
    console.log(JSON.stringify({
        step, lists: read.length, compile_ms: compiled, serialize_ms: serialized,
        serialized_kb: Math.round(Buffer.byteLength(selfie) / 1024),
        memory_before: before, memory_held: held, memory_peak: peak(),
    }));
} else if ( step === 'load' ) {
    const selfie = fs.readFileSync(args[0], 'utf8');
    const before = memory();
    const start = performance.now();
    await snfe.deserialize(selfie);
    const loaded = performance.now() - start;
    console.log(JSON.stringify({
        step, load_ms: loaded, memory_before: before, memory_held: memory(), memory_peak: peak(),
    }));
} else if ( step === 'match' ) {
    const [selfie, requests, out] = args;
    await snfe.deserialize(fs.readFileSync(selfie, 'utf8'));
    const rows = fs.readFileSync(requests, 'utf8').split('\n').filter(Boolean).map(line => {
        const { url, frameUrl, cpt } = JSON.parse(line);
        return { url, originURL: frameUrl, type: requestType(cpt) };
    });
    const times = [];
    const lines = [];
    let blocked = 0, excepted = 0, redirected = 0;
    // Twice, as main.rs does: the first warms, the second is timed.
    for ( let pass = 0; pass < 2; pass++ ) {
        for ( const row of rows ) {
            const start = process.hrtime.bigint();
            const verdict = snfe.matchRequest(row);
            const took = Number(process.hrtime.bigint() - start);
            if ( pass === 0 ) { continue; }
            times.push(took);
            let redirect = false;
            if ( verdict === 1 ) {
                blocked++;
                const found = snfe.matchAndFetchModifiers(row, 'redirect-rule');
                redirect = found !== undefined && found.length !== 0;
                if ( redirect ) { redirected++; }
            } else if ( verdict === 2 ) {
                excepted++;
            }
            lines.push(`${verdict}\t${redirect ? 'redirect' : ''}`);
        }
    }
    fs.writeFileSync(out, lines.join('\n') + '\n');
    const total = times.reduce((a, b) => a + b, 0);
    times.sort((a, b) => a - b);
    const at = q => times[Math.floor((times.length - 1) * q)] / 1000;
    console.log(JSON.stringify({
        step, requests: times.length, blocked, excepted, redirected,
        mean_us: total / times.length / 1000, p50_us: at(0.5), p99_us: at(0.99), max_us: at(1),
    }));
} else {
    console.error('node ubo.mjs UBO compile LISTS SELFIE | load SELFIE | match SELFIE REQUESTS OUT');
    process.exit(2);
}
