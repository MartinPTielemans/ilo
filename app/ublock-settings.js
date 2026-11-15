// ilo's, added to uBlock Origin's background page by app/fetch-ublock.sh as js/ilo.js.
//
// Settings › Ad blocking is ilo's own, and uBlock Origin applies it: the core's policy for it holds
// the lists to turn on and off and the user's own filters, under "ilo" (core/ad_block.cc). uBlock
// Origin reads its managed storage only as it starts, from a copy it kept, so this applies them
// as it starts and each time the policy changes, through the calls its own dashboard makes. An
// event, so nothing polls. What an administrator's toOverwrite sets stays as uBlock Origin applies
// it at launch.

import µb from './background.js';

async function apply() {
    let managed = {};
    try {
        managed = await chrome.storage.managed.get([ 'ilo', 'toOverwrite' ]) || {};
    } catch {
    }
    const ilo = managed.ilo;
    if ( ilo instanceof Object === false ) { return; }
    const admin = managed.toOverwrite instanceof Object ? managed.toOverwrite : {};
    let changed = false;
    const adminLists = Array.isArray(admin.filterLists) && admin.filterLists.length !== 0;
    if ( Array.isArray(ilo.on) && Array.isArray(ilo.off) && adminLists === false ) {
        const before = new Set(µb.selectedFilterLists);
        const after = new Set(before);
        for ( const key of ilo.on ) { after.add(key); }
        for ( const key of ilo.off ) { after.delete(key); }
        if ( after.size !== before.size || [ ...after ].some(key => before.has(key) === false) ) {
            µb.applyFilterListSelection({ toSelect: [ ...after ] });
            changed = true;
        }
    }
    if ( typeof ilo.filters === 'string' && Array.isArray(admin.filters) === false ) {
        const kept = await µb.loadUserFilters();
        const content = kept instanceof Object && typeof kept.content === 'string'
            ? kept.content
            : '';
        if ( content.trim() !== ilo.filters.trim() ) {
            await µb.saveUserFilters(ilo.filters);
            changed = true;
        }
    }
    // As the dashboard's Apply changes does, which fetches a list turned on that it hasn't kept.
    if ( changed ) {
        await µb.loadFilterLists();
    }
}

// One at a time, in the order they came.
let applying = Promise.resolve();
function applySoon() {
    applying = applying.then(apply, apply);
}

export default function settingsFromIlo() {
    chrome.storage.onChanged.addListener((changes, area) => {
        if ( area === 'managed' && changes.ilo ) { applySoon(); }
    });
    applySoon();
}
