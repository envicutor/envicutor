CREATE TABLE runtime (
    id SERIAL PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(255) UNIQUE NOT NULL,
    nix_shell TEXT NOT NULL,
    environment TEXT NOT NULL
);
