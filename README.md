nc 127.0.0.1 4000
QUERY SELECT COUNT(*) FROM data
SELECT SUM(metrics) FROM data WHERE timestamp = 1700003600

cargo run -- serve

cargo run --release -- web 8080 

# Then via TCP (nc 127.0.0.1 4000):
CREATE TABLE products (id INT, name TEXT, price FLOAT)
INSERT INTO products VALUES (1, 'Laptop', 999.99)
SELECT COUNT(*) FROM products
CREATE USER admin IDENTIFIED BY 'secret123'


CREATE TABLE pitem (id TEXT, name TEXT, price TEXT, category TEXT, stock TEXT)
INSERT INTO pitem VALUES ("1", 'Laptop', "999.99", "999.99", "999.99")