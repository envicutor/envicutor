const assert = require('assert');
const { sendRequest, BASE_URL } = require('./common');

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
    console.log('Listing runtimes (should have Python)');
    const res = await sendRequest('GET', `${BASE_URL}/runtimes`);

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 200);
    let body = JSON.parse(text);
    assert.deepEqual(body, [{ id: 1, name: 'Python' }]);
  }

  {
    console.log('Updating Nix');
    const res = await sendRequest('POST', `${BASE_URL}/update`);
    console.log(await res.text());
    assert.equal(res.status, 200);
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
  shellHook = ''
export multiline="multi
line"
export spaces="these spaces"
  '';
  nativeBuildInputs = with pkgs; [
      python3
  ];
}`,
      compile_script: '',
      run_script: 'exec python3 main.py',
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
      compile_script: 'exec g++ main.cpp',
      run_script: 'exec ./a.out',
      source_file_name: 'main.cpp'
    });

    console.log(await res.text());
    assert.equal(res.status, 200);
  }

  {
    console.log('Making an installation that will fail');
    const res = await sendRequest('POST', `${BASE_URL}/runtimes`, {
      name: 'Fake lang',
      nix_shell: `
{ pkgs ? import (
  fetchTarball {
    url="https://github.com/NixOS/nixpkgs/archive/72da83d9515b43550436891f538ff41d68eecc7f.tar.gz";
    sha256="177sws22nqkvv8am76qmy9knham2adfh3gv7hrjf6492z1mvy02y";
  }
) {} }:
pkgs.mkShell {
  shellHook = ''
  exit 1
  '';
  nativeBuildInputs = with pkgs; [];
}
`,
      compile_script: 'g++ main.cpp',
      run_script: './a.out',
      source_file_name: 'main.cpp'
    });

    console.log(await res.text());
    assert.equal(res.status, 400);
  }
})();
