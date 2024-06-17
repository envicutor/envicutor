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

    let text = await res.text();
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

    let text = await res.text();
    console.log(text);
    assert.equal(res.status, 200);
    let body = JSON.parse(text);
    assert.deepEqual(body, [{ id: 1, name: 'Python' }]);
  }
})();
