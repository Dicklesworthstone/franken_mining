export default {
  fetch(request, env) {
    const url = new URL(request.url);
    if (url.hostname === "www.frankenmining.com") {
      url.hostname = "frankenmining.com";
      return Response.redirect(url.href, 308);
    }
    return env.ASSETS.fetch(request);
  },
};
