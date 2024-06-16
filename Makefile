.PHONY: test

start:
	docker compose up -d --build
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
