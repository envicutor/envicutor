.PHONY: test

start-no-logs:
	docker compose up -d --build

start:
	make start-no-logs
	make logs

logs:
	docker compose logs -f

stop:
	docker compose down

migrate:
	docker volume rm -f envicutor_runtimes

stress:
	make stop
	make start-no-logs
	docker compose --profile test run --rm --build test "stress.js"
	make logs

test:
	make stop
	make migrate
	make start-no-logs
	docker compose --profile test run --rm --build test "installation.js"
	docker compose --profile test run --rm test "simple.js"
	make stop
	docker compose --profile test run --rm test "simple.js"
	docker compose --profile test run --rm test "complex.js"
	docker compose --profile test run --rm test "concurrency.js"
	/bin/bash ./test/check-residual.sh
	make logs
