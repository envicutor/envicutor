const assert = require('assert');
const { sendRequest, BASE_URL } = require('./common');

(async () => {
  {
    console.log('Listing runtimes (should have Python and C++)');
    const res = await sendRequest('GET', `${BASE_URL}/runtimes`);

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 200);
    let body = JSON.parse(text);
    body.sort((x, y) => x.id - y.id);
    assert.deepEqual(body, [
      { id: 2, name: 'Python' },
      { id: 3, name: 'C++' }
    ]);
  }

  {
    console.log('Executing Python code');
    const res = await sendRequest('POST', `${BASE_URL}/execute`, {
      runtime_id: 2,
      source_code: 'print(input())',
      input: 'Hello world'
    });

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 200);
    const body = JSON.parse(text);
    assert.equal(body.run.stdout, 'Hello world\n');
    assert.equal(body.run.stderr, '');
  }

  {
    console.log('Executing C++ code');
    const res = await sendRequest('POST', `${BASE_URL}/execute`, {
      runtime_id: 3,
      source_code: `
#include <iostream>
#include <string>

int main() {
  std::string in = "Hello";
  std::cout << in << '\\n';
  return 0;
}`
    });

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 200);
    const body = JSON.parse(text);
    assert.equal(body.run.stdout, 'Hello\n');
    assert.equal(body.run.stderr, '');
  }
})();
