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
time.sleep(0.5)
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
    assert.ok(total_time >= 500 && total_time < 1200, 'Total time was more than 1.2 seconds');
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
time.sleep(0.5)`
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
    assert.ok(total_time >= 1000 && total_time < 2000, 'Invalid total time');
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
    usleep(300000);
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
    assert.ok(total_time >= 600, 'Invalid total time');
  }

  {
    console.log(
      'Executing 32 submissions after a package installation has started (they should start after the installation)'
    );
    sendRequest('POST', `${BASE_URL}/runtimes`, {
      name: 'Fake lang',
      nix_shell: `{ pkgs ? import (
  fetchTarball {
    url="https://github.com/NixOS/nixpkgs/archive/72da83d9515b43550436891f538ff41d68eecc7f.tar.gz";
    sha256="177sws22nqkvv8am76qmy9knham2adfh3gv7hrjf6492z1mvy02y";
  }
) {} }:
pkgs.mkShell {
  shellHook = ''
  sleep 0.5
  exit 1
  '';
  nativeBuildInputs = with pkgs; [];
}`,
      compile_script: 'g++ main.cpp',
      run_script: './a.out',
      source_file_name: 'main.cpp'
    });

    await sleep(10);

    const promises = [];
    const start = new Date();
    for (let i = 0; i < 32; ++i) {
      promises.push(
        (async () => {
          await sendRequest('POST', `${BASE_URL}/execute`, {
            runtime_id: 2,
            source_code: `
import time
time.sleep(0.5)`
          });
          return new Date() - start;
        })()
      );
    }
    const before = new Date();
    const durations = await Promise.all(promises);
    const total_time = new Date() - before;
    for (const duration of durations) {
      assert.ok(duration >= 1000, 'Found a submission that finished before one second');
    }
    console.log(`Approximate time to run all submissions: ${total_time} ms`);
  }

  {
    console.log(
      'Running a package installation after executing 32 submissions has started (it should start after the executions finish)'
    );

    const promises = [];
    for (let i = 0; i < 32; ++i) {
      promises.push(
        sendRequest('POST', `${BASE_URL}/execute`, {
          runtime_id: 2,
          source_code: `
import time
time.sleep(0.5)`
        })
      );
    }
    await sleep(10);

    const start = new Date();
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
  sleep 0.5
  exit 1
  '';
  nativeBuildInputs = with pkgs; [];
}`,
      compile_script: 'g++ main.cpp',
      run_script: './a.out',
      source_file_name: 'main.cpp'
    });

    const duration = new Date() - start;
    console.log(`Time to finish installation: ${duration}`);
    assert.ok(duration >= 1000, 'Installation finished before 1 second');
  }

  {
    console.log(
      'Running a package installation after another installation has started (it should start after the installation finishes)'
    );

    sendRequest('POST', `${BASE_URL}/runtimes`, {
      name: 'Fake lang',
      nix_shell: `{ pkgs ? import (
  fetchTarball {
    url="https://github.com/NixOS/nixpkgs/archive/72da83d9515b43550436891f538ff41d68eecc7f.tar.gz";
    sha256="177sws22nqkvv8am76qmy9knham2adfh3gv7hrjf6492z1mvy02y";
  }
) {} }:
pkgs.mkShell {
  shellHook = ''
  sleep 0.5
  exit 1
  '';
  nativeBuildInputs = with pkgs; [];
}`,
      compile_script: 'g++ main.cpp',
      run_script: './a.out',
      source_file_name: 'main.cpp'
    });
    await sleep(10);

    const start = new Date();
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
  sleep 0.5
  exit 1
  '';
  nativeBuildInputs = with pkgs; [];
}`,
      compile_script: 'g++ main.cpp',
      run_script: './a.out',
      source_file_name: 'main.cpp'
    });

    const duration = new Date() - start;
    console.log(`Time to finish second installation: ${duration}`);
    assert.ok(duration >= 1000, 'Second installation finished before one second');
  }
})();
