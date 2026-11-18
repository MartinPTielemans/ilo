import WebKit

/// The script that finds a page's sign-in fields, in ilo's own world with the other page
/// scripts: the page can't call it, hear what it tells the app, or reach what fills its fields.
enum LoginScript {
  /// Whether the page's own frame shows a password field.
  static let message = "iloLogin"

  static func add(to content: WKUserContentController, handler: any WKScriptMessageHandler) {
    content.addUserScript(WKUserScript(source: source, injectionTime: .atDocumentStart, forMainFrameOnly: true, in: .defaultClient))
    content.add(handler, contentWorld: .defaultClient, name: message)
  }

  /// Tells the app when a password field that can be seen and typed into comes or goes, looking
  /// again only after the page changed or a field took the focus. `fill` puts a login into that
  /// field and the username field before it, as typing would for the page's own scripts, and
  /// only into the top frame's own document, at the origin the app says it is filling. Outside
  /// a form the username goes only into a field that says it is one, else nowhere.
  private static let source = """
    (() => {
      if (globalThis.iloLogin || window !== top) return;
      const post = shows => { try { webkit.messageHandlers.iloLogin.postMessage(shows); } catch (error) {} };
      const seen = element => {
        if (element.disabled || element.readOnly || !element.isConnected) return false;
        const box = element.getBoundingClientRect();
        // A field too small to see, or parked off the page's edge, is there for scripts, not people.
        if (box.width < 8 || box.height < 8 || box.right + scrollX <= 0 || box.bottom + scrollY <= 0) return false;
        return element.checkVisibility({ opacityProperty: true, visibilityProperty: true });
      };
      const auto = element => (element.getAttribute('autocomplete') || '').toLowerCase();
      // Not a field that asks for a new password, as a sign-up form's does.
      const passwordField = () => [...document.querySelectorAll('input[type="password"]')]
        .find(element => seen(element) && !/new-password/.test(auto(element)));
      // Whether a field says by itself that it takes a username.
      const says = element => /username|email/.test(auto(element)) || element.type === 'email'
        || /user|email|login|account/i.test(element.id + ' ' + (element.getAttribute('name') || ''));
      const usernameField = password => {
        // With no form to say which fields are the sign-in's, only one that says it takes a
        // username is: the field before the password may be the page's search box.
        const fields = [...(password.form ? password.form.elements : document.querySelectorAll('input'))]
          .filter(element => element.tagName === 'INPUT' && ['text', 'email', 'tel'].includes(element.type) && seen(element)
            && (password.form || says(element)));
        const named = fields.find(element => /(^|\\s)(username|email)(\\s|$)/.test(auto(element)));
        if (named) return named;
        // Else the last text field before the password.
        let before = null;
        for (const field of fields) {
          if (field.compareDocumentPosition(password) & Node.DOCUMENT_POSITION_FOLLOWING) before = field;
        }
        return before;
      };

      let last = null;
      let timer = 0;
      const look = () => {
        timer = 0;
        const shows = !!passwordField();
        if (shows === last) return;
        last = shows;
        post(shows);
      };
      const soon = () => { if (!timer) timer = setTimeout(look, 300); };
      new MutationObserver(soon).observe(document, { childList: true, subtree: true, attributes: true, attributeFilter: ['type'] });
      document.addEventListener('DOMContentLoaded', look);
      addEventListener('load', look);
      // A form the page had hidden shows by the time one of its fields takes the focus.
      addEventListener('focusin', soon, true);
      // A page brought back whole from the back-forward cache says so again.
      addEventListener('pageshow', event => { if (event.persisted) { last = null; look(); } });

      // The fields filled, for the script that keeps unsent text, in this world too, to pass over.
      const filled = globalThis.iloFilled || (globalThis.iloFilled = new WeakSet());
      const set = (element, value) => {
        filled.add(element);
        // The field's own setter, then the events a page's script waits for after typing.
        Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(element, value);
        element.dispatchEvent(new Event('input', { bubbles: true }));
        element.dispatchEvent(new Event('change', { bubbles: true }));
      };
      const fill = (origin, username, password) => {
        if (window !== top || location.origin !== origin) return 'moved';
        const field = passwordField();
        if (!field) return 'gone';
        const user = usernameField(field);
        if (user && username) set(user, username);
        set(field, password);
        return 'filled';
      };
      globalThis.iloLogin = { fill };
    })();
    """
}
