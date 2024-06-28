const assert = require('assert');
const {
  sendRequest,
  BASE_URL,
  RUN_WALL_TIME,
  RUN_CPU_TIME,
  RUN_MEMORY,
  RUN_EXTRA_TIME,
  RUN_MAX_OPEN_FILES,
  RUN_MAX_FILE_SIZE,
  RUN_MAX_NUMBER_OF_PROCESSES
} = require('./common');

(async () => {
  {
    console.log('Checking the environment variables in Python');
    const res = await sendRequest('POST', `${BASE_URL}/execute`, {
      runtime_id: 2,
      source_code: `import os
print(os.environ["multiline"] == "multi\\nline")
print(os.environ["spaces"] == "these spaces")
`
    });

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 200);
    const body = JSON.parse(text);
    assert.equal(body.run.stdout, 'True\nTrue\n');
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

  {
    console.log('Installing Bash');
    const res = await sendRequest('POST', `${BASE_URL}/runtimes`, {
      name: 'Bash',
      nix_shell: `
{ pkgs ? import (
  fetchTarball {
    url="https://github.com/NixOS/nixpkgs/archive/72da83d9515b43550436891f538ff41d68eecc7f.tar.gz";
    sha256="177sws22nqkvv8am76qmy9knham2adfh3gv7hrjf6492z1mvy02y";
  }
) {} }:
pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
      bash
  ];
}`,
      compile_script: '',
      run_script: 'bash main.sh',
      source_file_name: 'main.sh'
    });

    console.log(await res.text());
    assert.equal(res.status, 200);
  }

  // https://github.com/ioi/isolate/issues/158
  {
    console.log('Creating a directory that can not be removed (Envicutor shall remove it)');
    const res = await sendRequest('POST', `${BASE_URL}/execute`, {
      runtime_id: 4,
      source_code: 'mkdir test && chmod 0700 test && touch test/some-file && echo directory created'
    });

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 200);
    const body = JSON.parse(text);
    assert.equal(body.run.stdout, 'directory created\n');
  }

  {
    console.log('Executing over-cpu-time-limit Python code');
    const res = await sendRequest('POST', `${BASE_URL}/execute`, {
      runtime_id: 2,
      source_code: `
x = 0
while True:
  x += 1
`,
      run_limits: {
        cpu_time: 0.1,
        extra_time: 0
      }
    });

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 200);
    const body = JSON.parse(text);
    assert.equal(body.run.exit_status, 'TO');
  }

  {
    console.log('Executing over-memory-limit C++ code');
    const res = await sendRequest('POST', `${BASE_URL}/execute`, {
      runtime_id: 3,
      source_code: `const int N = 14e6;
char mem[N];

int main()
{
	for (int i = 0; i < N; ++i)
		mem[i] = 1;
	return 0;
}
`,
      run_limits: {
        memory: 13000
      }
    });

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 200);
    const body = JSON.parse(text);
    assert.equal(body.run.exit_signal, 9);
  }

  {
    console.log('Executing under-memory-limit C++ code');
    const res = await sendRequest('POST', `${BASE_URL}/execute`, {
      runtime_id: 3,
      source_code: `const int N = 11e6;
char mem[N];

int main()
{
	for (int i = 0; i < N; ++i)
		mem[i] = 1;
	return 0;
}
`,
      run_limits: {
        memory: 13000
      }
    });

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 200);
    const body = JSON.parse(text);
    assert.equal(body.run.exit_code, 0);
  }

  {
    console.log('Executing over-wall-time-limit Python code');
    const res = await sendRequest('POST', `${BASE_URL}/execute`, {
      runtime_id: 2,
      source_code: `
import time
time.sleep(0.5)`,
      run_limits: {
        wall_time: 0.3,
        extra_time: 0
      }
    });

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 200);
    const body = JSON.parse(text);
    assert.equal(body.run.exit_status, 'TO');
  }

  {
    console.log('Executing below-wall-time-limit Python code');
    const res = await sendRequest('POST', `${BASE_URL}/execute`, {
      runtime_id: 2,
      source_code: `
import time
time.sleep(0.1)`,
      run_limits: {
        wall_time: 0.3,
        extra_time: 0
      }
    });

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 200);
    const body = JSON.parse(text);
    assert.equal(body.run.exit_code, 0);
  }

  {
    console.log('Executing over-number-of-processes-limit Python code');
    const res = await sendRequest('POST', `${BASE_URL}/execute`, {
      runtime_id: 2,
      source_code: `
import os

while True:
    os.fork()`,
      run_limits: {
        max_number_of_processes: 4
      }
    });

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 200);
    const body = JSON.parse(text);
    assert.equal(body.run.exit_code, 1);
  }

  {
    console.log('Executing below-number-of-processes-limit Python code');
    const res = await sendRequest('POST', `${BASE_URL}/execute`, {
      runtime_id: 2,
      source_code: `import subprocess
s = subprocess.Popen(["echo", "hello"], stdout=subprocess.PIPE)
stdout, _ = s.communicate()
print(stdout.decode().strip())`,
      run_limits: {
        max_number_of_processes: 2
      }
    });

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 200);
    const body = JSON.parse(text);
    assert.equal(body.run.exit_code, 0);
    assert.equal(body.run.stdout, 'hello\n');
  }

  {
    console.log('Executing above-number-of-processes-limit python code');
    const res = await sendRequest('POST', `${BASE_URL}/execute`, {
      runtime_id: 2,
      source_code: `import subprocess
s = subprocess.Popen(["echo", "hello"], stdout=subprocess.PIPE)
stdout, _ = s.communicate()
print(stdout.decode().strip())`,
      run_limits: {
        max_number_of_processes: 1
      }
    });

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 200);
    const body = JSON.parse(text);
    assert.equal(body.run.exit_code, 1);
  }

  {
    console.log('Executing above-number-of-processes-limit python code using threads');
    const res = await sendRequest('POST', `${BASE_URL}/execute`, {
      runtime_id: 2,
      source_code: `import threading
def test():
    print("yo")
t=threading.Thread(target=test)
t.start()`,
      run_limits: {
        max_number_of_processes: 1
      }
    });

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 200);
    const body = JSON.parse(text);
    assert.equal(body.run.exit_code, 1);
  }

  {
    console.log('Aborting mid-submission (should not cause Envicutor errors)');
    const controller = new AbortController();

    setTimeout(() => {
      controller.abort();
    }, 20);

    try {
      await sendRequest(
        'POST',
        `${BASE_URL}/execute`,
        {
          runtime_id: 2,
          source_code: `
  import time

  time.sleep(5)
  `
        },
        controller.signal
      );
    } catch (e) {}
  }

  {
    console.log('Executing Python code with invalid run wall_time');
    const res = await sendRequest('POST', `${BASE_URL}/execute`, {
      runtime_id: 2,
      source_code: 'print(input())',
      input: 'Hello world',
      run_limits: {
        wall_time: RUN_WALL_TIME + 1
      }
    });

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 400);
    const body = JSON.parse(text);
    assert.equal(
      body.message,
      `Invalid run limits: wall_time can't exceed ${RUN_WALL_TIME} seconds`
    );
  }

  {
    console.log('Executing Python code with invalid run cpu_time');
    const res = await sendRequest('POST', `${BASE_URL}/execute`, {
      runtime_id: 2,
      source_code: 'print(input())',
      input: 'Hello world',
      run_limits: {
        cpu_time: RUN_CPU_TIME + 1
      }
    });

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 400);
    const body = JSON.parse(text);
    assert.equal(body.message, `Invalid run limits: cpu_time can't exceed ${RUN_CPU_TIME} seconds`);
  }

  {
    console.log('Executing Python code with invalid run memory');
    const res = await sendRequest('POST', `${BASE_URL}/execute`, {
      runtime_id: 2,
      source_code: 'print(input())',
      input: 'Hello world',
      run_limits: {
        memory: RUN_MEMORY + 1
      }
    });

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 400);
    const body = JSON.parse(text);
    assert.equal(body.message, `Invalid run limits: memory can't exceed ${RUN_MEMORY} kilobytes`);
  }

  {
    console.log('Executing Python code with invalid run extra_time');
    const res = await sendRequest('POST', `${BASE_URL}/execute`, {
      runtime_id: 2,
      source_code: 'print(input())',
      input: 'Hello world',
      run_limits: {
        extra_time: RUN_EXTRA_TIME + 1
      }
    });

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 400);
    const body = JSON.parse(text);
    assert.equal(
      body.message,
      `Invalid run limits: extra_time can't exceed ${RUN_EXTRA_TIME} seconds`
    );
  }

  {
    console.log('Executing Python code with invalid run max_open_files');
    const res = await sendRequest('POST', `${BASE_URL}/execute`, {
      runtime_id: 2,
      source_code: 'print(input())',
      input: 'Hello world',
      run_limits: {
        max_open_files: RUN_MAX_OPEN_FILES + 1
      }
    });

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 400);
    const body = JSON.parse(text);
    assert.equal(
      body.message,
      `Invalid run limits: max_open_files can't exceed ${RUN_MAX_OPEN_FILES}`
    );
  }

  {
    console.log(
      'Executing Python code with a higher max_open_files limit (should not be able to open all of them)'
    );
    const res = await sendRequest('POST', `${BASE_URL}/execute`, {
      runtime_id: 2,
      source_code: `
import os

input_data = input()
open_files = []

for i in range(50):
    try:
        file = open(f"file{i}.txt", "w")
        open_files.append(file)
        print(f"Opened file{i}.txt")
    except Exception as e:
        print(f"Failed to open file{i}.txt", file=sys.stderr)

for file in open_files:
    try:
        file.write(input_data)
        file.close()
    except Exception as e:
        print(f"Failed to write to or close file {file.name}")
`,
      input: 'Hello world',
      run_limits: {
        max_open_files: 50
      }
    });

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 200);
    const body = JSON.parse(text);
    assert.equal(body.run.exit_code, 1);
  }

  {
    console.log('Executing Python code with a lower max_open_files limit');
    const res = await sendRequest('POST', `${BASE_URL}/execute`, {
      runtime_id: 2,
      source_code: `
import os

input_data = input()
open_files = []

for i in range(40):
    try:
        file = open(f"file{i}.txt", "w")
        open_files.append(file)
        print(f"Opened file{i}.txt")
    except Exception as e:
        print(f"Failed to open file{i}.txt", file=sys.stderr)

for file in open_files:
    try:
        file.write(input_data)
        file.close()
    except Exception as e:
        print(f"Failed to write to or close file {file.name}")
`,
      input: 'Hello world',
      run_limits: {
        max_open_files: 50
      }
    });

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 200);
    const body = JSON.parse(text);
    assert.equal(body.run.stderr, '');
  }

  {
    console.log('Executing Python code with invalid run max_file_size');
    const res = await sendRequest('POST', `${BASE_URL}/execute`, {
      runtime_id: 2,
      source_code: 'print(input())',
      input: 'Hello world',
      run_limits: {
        max_file_size: RUN_MAX_FILE_SIZE + 1
      }
    });

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 400);
    const body = JSON.parse(text);
    assert.equal(
      body.message,
      `Invalid run limits: max_file_size can't exceed ${RUN_MAX_FILE_SIZE} kilobytes`
    );
  }

  {
    console.log('Executing over-file-size-limit Python code');
    const res = await sendRequest('POST', `${BASE_URL}/execute`, {
      runtime_id: 2,
      source_code: `
import os

file_path = "large_file.txt"
data = 'A' * (1024 * 1024 * 5)  # 5 MB string
with open(file_path, "w") as file:
    file.write(data)
    print(f"File {file_path} created successfully.")

`,
      run_limits: {
        max_file_size: 1024 * 3 // 3 MB
      }
    });

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 200);
    const body = JSON.parse(text);
    assert.equal(body.run.exit_code, 1);
  }

  {
    console.log('Executing under-file-size-limit Python code');
    const res = await sendRequest('POST', `${BASE_URL}/execute`, {
      runtime_id: 2,
      source_code: `
import os

file_path = "small_file.txt"
data = 'A' * 1024  # 1 KB string
with open(file_path, "w") as file:
  file.write(data)
  print(f"File {file_path} created successfully.")
`,
      run_limits: {
        max_file_size: 1024 * 3 // 3 MB
      }
    });

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 200);
    const body = JSON.parse(text);
    assert.equal(body.run.exit_code, 0);
  }

  {
    console.log('Executing Python code with invalid run max_number_of_processes');
    const res = await sendRequest('POST', `${BASE_URL}/execute`, {
      runtime_id: 2,
      source_code: 'print(input())',
      input: 'Hello world',
      run_limits: {
        max_number_of_processes: RUN_MAX_NUMBER_OF_PROCESSES + 1
      }
    });

    const text = await res.text();
    console.log(text);
    assert.equal(res.status, 400);
    const body = JSON.parse(text);
    assert.equal(
      body.message,
      `Invalid run limits: max_number_of_processes can't exceed ${RUN_MAX_NUMBER_OF_PROCESSES}`
    );
  }
})();
