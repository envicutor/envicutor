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
module.exports.RUN_WALL_TIME = parseFloat(process.env['RUN_WALL_TIME']);
module.exports.RUN_CPU_TIME = parseFloat(process.env['RUN_CPU_TIME']);
module.exports.RUN_EXTRA_TIME = parseFloat(process.env['RUN_EXTRA_TIME']);
module.exports.RUN_MEMORY = parseInt(process.env['RUN_MEMORY']);
module.exports.RUN_MAX_OPEN_FILES = parseInt(process.env['RUN_MAX_OPEN_FILES']);
module.exports.RUN_MAX_FILE_SIZE = parseInt(process.env['RUN_MAX_FILE_SIZE']);
module.exports.RUN_MAX_NUMBER_OF_PROCESSES = parseInt(process.env['RUN_MAX_NUMBER_OF_PROCESSES']);
module.exports.MAX_CONCURRENT_SUBMISSIONS = parseInt(process.env['MAX_CONCURRENT_SUBMISSIONS']);

module.exports.sleep = async (t) => await new Promise((res) => setTimeout(res, t));
