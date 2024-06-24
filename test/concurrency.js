const assert = require('assert');
const { sendRequest, BASE_URL, sleep, MAX_CONCURRENT_SUBMISSIONS } = require('./common');

(async () => {
  {
    console.log('Executing MAX_CONCURRENT_SUBMISSIONS Python submissions in parallel');
    const promises = [];
    const before = new Date();
    for (let i = 0; i < MAX_CONCURRENT_SUBMISSIONS; ++i) {
      promises.push(
        sendRequest('POST', `${BASE_URL}/execute`, {
          runtime_id: 2,
          source_code: `
import time
time.sleep(0.3)
print(input())`,
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
    console.log(`Approximate time to run all submissions: ${total_time} ms`);
    assert.ok(
      total_time >= 300 && total_time < MAX_CONCURRENT_SUBMISSIONS * 300,
      'Invalid total time'
    );
  }

  {
    console.log(
      'Executing MAX_CONCURRENT_SUBMISSIONS * 2 Python submissions in parallel (the second MAX_CONCURRENT_SUBMISSIONS should be blocked for some time)'
    );
    const promises = [];
    const before = new Date();
    for (let i = 0; i < MAX_CONCURRENT_SUBMISSIONS * 2; ++i) {
      promises.push(
        sendRequest('POST', `${BASE_URL}/execute`, {
          runtime_id: 2,
          source_code: `
import time
time.sleep(0.3)`
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
      assert.equal(body.run.exit_code, 0);
    }
    const total_time = after - before;
    console.log(`Approximate time to run all submissions: ${total_time} ms`);
    assert.ok(
      total_time >= 600 && total_time < MAX_CONCURRENT_SUBMISSIONS * 600,
      'Invalid total time'
    );
  }

  {
    console.log(
      'Executing MAX_CONCURRENT_SUBMISSIONS * 2 C++ submissions in parallel (the second MAX_CONCURRENT_SUBMISSIONS should be blocked for some time)'
    );
    const promises = [];
    const before = new Date();
    for (let i = 0; i < MAX_CONCURRENT_SUBMISSIONS * 2; ++i) {
      promises.push(
        sendRequest('POST', `${BASE_URL}/execute`, {
          runtime_id: 3,
          source_code: `
#include <unistd.h>

int main() {
    usleep(100000);
    return 0;
}`
        })
      );
    }
    const responses = await Promise.all(promises);
    const total_time = new Date() - before;
    for (const res of responses) {
      const text = await res.text();
      assert.equal(res.status, 200);
      const body = JSON.parse(text);
      assert.equal(body.run.stdout, '');
      assert.equal(body.run.stderr, '');
      assert.equal(body.run.exit_code, 0);
    }
    console.log(`Approximate time to run all submissions: ${total_time} ms`);
    assert.ok(
      total_time >= 200 && total_time < MAX_CONCURRENT_SUBMISSIONS * 200,
      'Invalid total time'
    );
  }

  {
    console.log(
      'Executing Math.ceil(MAX_CONCURRENT_SUBMISSIONS / 2) submissions after a package installation has started (they should start after the installation)'
    );

    const installation_promise = (async () => {
      await sendRequest('POST', `${BASE_URL}/runtimes`, {
        name: 'Fake lang',
        nix_shell: `{ pkgs ? import (
    fetchTarball {
      url="https://github.com/NixOS/nixpkgs/archive/72da83d9515b43550436891f538ff41d68eecc7f.tar.gz";
      sha256="177sws22nqkvv8am76qmy9knham2adfh3gv7hrjf6492z1mvy02y";
    }
  ) {} }:
  pkgs.mkShell {
    shellHook = ''
    sleep 0.3
    exit 1
    '';
    nativeBuildInputs = with pkgs; [];
  }`,
        compile_script: 'g++ main.cpp',
        run_script: './a.out',
        source_file_name: 'main.cpp'
      });
      return new Date();
    })();

    await sleep(10);

    const promises = [];
    for (let i = 0; i < Math.ceil(MAX_CONCURRENT_SUBMISSIONS / 2); ++i) {
      promises.push(
        (async () => {
          await sendRequest('POST', `${BASE_URL}/execute`, {
            runtime_id: 2,
            source_code: 'print("Hello world")'
          });
          return new Date();
        })()
      );
    }

    const installation_finish = await installation_promise;
    const before = new Date();
    const execution_finishes = await Promise.all(promises);
    const total_time = new Date() - before;
    for (const finish of execution_finishes) {
      assert.ok(
        finish >= installation_finish,
        'Found a submission that finished before the installation'
      );
    }
    console.log(`Approximate time to run all submissions: ${total_time} ms`);
  }

  {
    console.log(
      'Running a package installation after executing Math.ceil(MAX_CONCURRENT_SUBMISSIONS / 2) submissions has started (it should start after the executions finish)'
    );

    const execution_promises = [];
    for (let i = 0; i < Math.ceil(MAX_CONCURRENT_SUBMISSIONS / 2); ++i) {
      execution_promises.push(
        (async () => {
          await sendRequest('POST', `${BASE_URL}/execute`, {
            runtime_id: 2,
            source_code: `
  import time
  time.sleep(0.3)`
          });
          return new Date();
        })()
      );
    }
    await sleep(10);

    const installation_promise = (async () => {
      await sendRequest('POST', `${BASE_URL}/runtimes`, {
        name: 'Fake lang',
        nix_shell: `{ pkgs ? import (
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
  }`,
        compile_script: 'g++ main.cpp',
        run_script: './a.out',
        source_file_name: 'main.cpp'
      });
      return new Date();
    })();

    const execution_finishes = await Promise.all(execution_promises);
    const last_execution_finish = Math.max(...execution_finishes);
    const before = new Date();
    const installation_finish = await installation_promise;
    const duration = new Date() - before;

    console.log(`Time to finish installation: ${duration}`);
    assert.ok(
      installation_finish > last_execution_finish,
      'Installation finished before last execution'
    );
  }

  {
    console.log(
      'Running a package installation after another installation has started (it should start after the installation finishes)'
    );
    const first_promise = (async () => {
      await sendRequest('POST', `${BASE_URL}/runtimes`, {
        name: 'Fake lang',
        nix_shell: `{ pkgs ? import (
    fetchTarball {
      url="https://github.com/NixOS/nixpkgs/archive/72da83d9515b43550436891f538ff41d68eecc7f.tar.gz";
      sha256="177sws22nqkvv8am76qmy9knham2adfh3gv7hrjf6492z1mvy02y";
    }
  ) {} }:
  pkgs.mkShell {
    shellHook = ''
    sleep 0.3
    exit 1
    '';
    nativeBuildInputs = with pkgs; [];
  }`,
        compile_script: 'g++ main.cpp',
        run_script: './a.out',
        source_file_name: 'main.cpp'
      });
      return new Date();
    })();

    await sleep(10);

    const second_promise = (async () => {
      await sendRequest('POST', `${BASE_URL}/runtimes`, {
        name: 'Fake lang',
        nix_shell: `{ pkgs ? import (
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
  }`,
        compile_script: 'g++ main.cpp',
        run_script: './a.out',
        source_file_name: 'main.cpp'
      });
      return new Date();
    })();

    const first_finish = await first_promise;
    const before = new Date();
    const second_finish = await second_promise;
    const duration = new Date() - before;
    console.log(`Time to finish second installation: ${duration}`);
    assert.ok(
      second_finish >= first_finish,
      'Second installation finished before first installation'
    );
  }
})();
