set -x
docker compose exec envicutor /bin/bash -c 'if [[ -z "$(ls /var/local/lib/isolate && ls /envicutor/tmp && ls /sys/fs/cgroup/isolate/box-*)" ]]; then exit 0; else exit 1; fi' || (echo found residual files && exit 1)
