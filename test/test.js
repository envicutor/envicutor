const assert = require('assert');

const BASE_URL = 'http://envicutor:5000';

const sendRequest = (method, url, body) => {
  const opts = {
    method,
    headers: {
      'Content-Type': 'application/json'
    }
  };
  if (method.toLowerCase() !== 'get' && method.toLowerCase() !== 'delete')
    opts.body = JSON.stringify(body);
  return fetch(url, opts);
};

(async () => {
  {
    console.log('Installing Python');
    const res = await sendRequest('POST', `${BASE_URL}/runtimes`, {
      name: 'Python',
      nix_shell: `
{ pkgs ? import (
  fetchTarball {
    url="https://github.com/NixOS/nixpkgs/archive/72da83d9515b43550436891f538ff41d68eecc7f.tar.gz";
    sha256="177sws22nqkvv8am76qmy9knham2adfh3gv7hrjf6492z1mvy02y";
  }
) {} }:
pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
      python3
  ];
}`,
      compile_script: '',
      run_script: 'python3 main.py',
      source_file_name: 'main.py'
    });

    console.log(await res.text());
    assert.equal(res.status, 200);
  }

  {
    console.log('Installing Python again (should fail)');
    const res = await sendRequest('POST', `${BASE_URL}/runtimes`, {
      name: 'Python',
      nix_shell: `
{ pkgs ? import (
  fetchTarball {
    url="https://github.com/NixOS/nixpkgs/archive/72da83d9515b43550436891f538ff41d68eecc7f.tar.gz";
    sha256="177sws22nqkvv8am76qmy9knham2adfh3gv7hrjf6492z1mvy02y";
  }
) {} }:
pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
      python3
  ];
}`,
      compile_script: '',
      run_script: 'python3 main.py',
      source_file_name: 'main.py'
    });

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 400);
    let body = JSON.parse(text);
    assert.deepEqual(body, {
      message: 'A runtime with this name already exists'
    });
  }

  {
    console.log('Updating Nix');
    const res = await sendRequest('POST', `${BASE_URL}/update`);
    console.log(await res.text());
    assert.equal(res.status, 200);
  }

  {
    console.log('Listing runtimes (should have Python)');
    const res = await sendRequest('GET', `${BASE_URL}/runtimes`);

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 200);
    let body = JSON.parse(text);
    assert.deepEqual(body, [{ id: 1, name: 'Python' }]);
  }

  {
    console.log('Deleting runtime with id 2 (invalid)');
    const res = await sendRequest('DELETE', `${BASE_URL}/runtimes/2`);

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 404);
    let body = JSON.parse(text);
    assert.deepEqual(body, { message: 'Could not find the specified runtime' });
  }

  {
    console.log('Deleting runtime with id 1 (delete Python)');
    const res = await sendRequest('DELETE', `${BASE_URL}/runtimes/1`);

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 200);
  }

  {
    console.log('Listing runtimes (should be empty)');
    const res = await sendRequest('GET', `${BASE_URL}/runtimes`);

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 200);
    let body = JSON.parse(text);
    assert.deepEqual(body, []);
  }

  {
    console.log('Installing Python');
    const res = await sendRequest('POST', `${BASE_URL}/runtimes`, {
      name: 'Python',
      nix_shell: `
{ pkgs ? import (
  fetchTarball {
    url="https://github.com/NixOS/nixpkgs/archive/72da83d9515b43550436891f538ff41d68eecc7f.tar.gz";
    sha256="177sws22nqkvv8am76qmy9knham2adfh3gv7hrjf6492z1mvy02y";
  }
) {} }:
pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
      python3
  ];
}`,
      compile_script: '',
      run_script: 'python3 main.py',
      source_file_name: 'main.py'
    });

    console.log(await res.text());
    assert.equal(res.status, 200);
  }

  {
    console.log('Installing C++ via gcc');
    const res = await sendRequest('POST', `${BASE_URL}/runtimes`, {
      name: 'C++',
      nix_shell: `
{ pkgs ? import (
  fetchTarball {
    url="https://github.com/NixOS/nixpkgs/archive/72da83d9515b43550436891f538ff41d68eecc7f.tar.gz";
    sha256="177sws22nqkvv8am76qmy9knham2adfh3gv7hrjf6492z1mvy02y";
  }
) {} }:
pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
      gcc
  ];
}`,
      compile_script: 'g++ main.cpp',
      run_script: './a.out',
      source_file_name: 'main.cpp'
    });

    console.log(await res.text());
    assert.equal(res.status, 200);
  }

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
  std::string in;
  std::cin >> in;
  std::cout << in << '\\n';
  return 0;
}`,
      input: 'Hello world'
    });

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 200);
    const body = JSON.parse(text);
    assert.equal(body.run.stdout, 'Hello\n');
    assert.equal(body.run.stderr, '');
  }

  {
    console.log('Executing C++ code with a compile error (run result should be null)');
    const res = await sendRequest('POST', `${BASE_URL}/execute`, {
      runtime_id: 3,
      source_code: `
#include <iostream>
#include <string>

int main()x {
  std::string in;
  std::cin >> in;
  std::cout << in << '\\n';
  return 0;
}`,
      input: 'Hello world'
    });

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 200);
    const body = JSON.parse(text);
    assert.equal(body.run, null);
    assert.equal(body.compile.exit_code, 1);
  }

  {
    console.log('Executing erroneous Python code');
    const res = await sendRequest('POST', `${BASE_URL}/execute`, {
      runtime_id: 2,
      source_code: 'print(input()x)',
      input: 'Hello world'
    });

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 200);
    const body = JSON.parse(text);
    assert.equal(body.run.exit_code, 1);
  }
})();
