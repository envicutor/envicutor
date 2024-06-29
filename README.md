# Prerequisites

- Docker
- Docker compose
- Make
- cgroup v2 enabled

# Running

To start Envicutor:

```bash
make start
```

To run API tests:

```bash
make test
```

To run stress tests (must be run after API tests):

```bash
make stress
```

To stop Envicutor:

```bash
make stop
```
