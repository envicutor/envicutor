const assert = require('assert');
const { sendRequest, BASE_URL } = require('./common');

(async () => {
  {
    console.log('Executing 64 Python submissions in parallel');
    const promises = [];
    const before = new Date();
    for (let i = 0; i < 64; ++i) {
      promises.push(
        sendRequest('POST', `${BASE_URL}/execute`, {
          runtime_id: 2,
          source_code: 'print(input())',
          input: 'Hello world'
        })
      );
    }
    const responses = await Promise.all(promises);
    const after = new Date();
    for (const res of responses) {
      const text = await res.text();
      assert.equal(res.status, 200);
      const body = JSON.parse(text);
      assert.equal(body.run.stdout, 'Hello world\n');
      assert.equal(body.run.stderr, '');
    }
    const total_time = after - before;
    console.log(`Approximate time to run all submissions: ${after - before} ms`);
    assert.ok(total_time < 1000, 'Total time was more than 1 second');
  }

  {
    console.log(
      'Executing 128 Python submissions in parallel (should be around 2 seconds if max concurrent submissions is 64)'
    );
    const promises = [];
    const before = new Date();
    for (let i = 0; i < 128; ++i) {
      promises.push(
        sendRequest('POST', `${BASE_URL}/execute`, {
          runtime_id: 2,
          source_code: `
import time
time.sleep(1)`
        })
      );
    }
    const responses = await Promise.all(promises);
    const after = new Date();
    for (const res of responses) {
      const text = await res.text();
      assert.equal(res.status, 200);
      const body = JSON.parse(text);
      assert.equal(body.run.stdout, '');
      assert.equal(body.run.stderr, '');
    }
    const total_time = after - before;
    console.log(`Approximate time to run all submissions: ${after - before} ms`);
    assert.ok(total_time < 3200, 'Total time was more than 3.2 seconds');
  }
})();
