// Put in front of every sealed page: its one way out, a port the host hands it.
(() => {
  let port = null, n = 0, ready;
  const waiting = new Map(), onShown = [];
  const isReady = new Promise((r) => (ready = r));
  addEventListener('message', (e) => {
    if (port || e.source !== parent || e.data?.siteApi !== 'hello') return;
    port = e.ports[0];
    port.onmessage = ({ data }) => {
      if (data.type === 'shown') onShown.forEach((f) => f());
      else if (waiting.has(data.id)) { waiting.get(data.id)(data.answer); waiting.delete(data.id); }
    };
    ready();
  });
  window.siteApi = {
    async call(api, params = {}) {
      await isReady;
      const id = ++n;
      return new Promise((r) => { waiting.set(id, r); port.postMessage({ type: 'call', id, api, params }); });
    },
    onShown(f) { onShown.push(f); },
    open(answer) { port?.postMessage({ type: 'open', open: answer.open }); },
  };
})();
