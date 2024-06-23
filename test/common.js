module.exports.sendRequest = (method, url, body, signal) => {
  const opts = {
    method,
    signal,
    headers: {
      'Content-Type': 'application/json'
    }
  };
  if (method.toLowerCase() !== 'get' && method.toLowerCase() !== 'delete')
    opts.body = JSON.stringify(body);
  return fetch(url, opts);
};

module.exports.BASE_URL = 'http://envicutor:5000';

module.exports.sleep = async (t) => await new Promise((res) => setTimeout(res, t));
