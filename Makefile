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

stop-test:
	docker compose --profile test down

test:
	make stop-test
	make stop
	make migrate
	docker compose --profile test up -d --build
	docker compose --profile test logs -f
