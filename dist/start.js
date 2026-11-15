// Checks that the site answers, then opens the sign-in page. Signed-in people
// are sent on to their dashboard by the site. If the site cannot be reached,
// the offline message with a Retry button is shown instead.
(function () {
  var SITE = "https://www.mentorintell.com";
  var START = SITE + "/login";
  var checking = document.getElementById("checking");
  var offline = document.getElementById("offline");
  var retry = document.getElementById("retry");

  function show(el) {
    checking.hidden = el !== checking;
    offline.hidden = el !== offline;
  }

  function check() {
    show(checking);
    retry.disabled = true;
    if (navigator.onLine === false) {
      show(offline);
      retry.disabled = false;
      return;
    }
    var controller = new AbortController();
    var timer = setTimeout(function () { controller.abort(); }, 10000);
    // no-cors: we only need to know the server answered, not read the reply.
    fetch(SITE + "/mobile-release.json", { mode: "no-cors", cache: "no-store", signal: controller.signal })
      .then(function () {
        clearTimeout(timer);
        location.replace(START);
      })
      .catch(function () {
        clearTimeout(timer);
        show(offline);
        retry.disabled = false;
        retry.focus();
      });
  }

  retry.addEventListener("click", check);
  window.addEventListener("online", function () {
    if (!offline.hidden) check();
  });
  check();
})();
