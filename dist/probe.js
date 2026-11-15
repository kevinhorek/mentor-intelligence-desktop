// Hidden page. Hands the webview's default user agent to the shell by
// navigating to a reserved .invalid address that the shell intercepts.
location.replace("https://ua-probe.invalid/?ua=" + encodeURIComponent(navigator.userAgent));
