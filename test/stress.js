const assert = require('assert');
const { sendRequest, BASE_URL } = require('./common');

(async () => {
  {
    console.log('Executing 5000 Python submissions in parallel');
    const promises = [];
    for (let i = 0; i < 5000; ++i) {
      promises.push(
        sendRequest('POST', `${BASE_URL}/execute`, {
          runtime_id: 2,
          source_code: 'print(input())',
          input: 'Hello world'
        })
      );
    }
    const before = new Date();
    const responses = await Promise.all(promises);
    console.log(`Time taken: ${new Date() - before} ms`);
    for (const res of responses) {
      const text = await res.text();
      assert.equal(res.status, 200);
      const body = JSON.parse(text);
      assert.equal(body.run.stdout, 'Hello world\n');
      assert.equal(body.run.stderr, '');
    }
  }
})();
