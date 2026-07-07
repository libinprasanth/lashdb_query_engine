import socket
import random
import string
import time

HOST = "127.0.0.1"
PORT = 4000

TOTAL_ROWS = 5000000     # Adjust as needed
BATCH_SIZE = 10000

categories = [
    "Electronics",
    "Books",
    "Fashion",
    "Sports",
    "Home",
    "Toys",
    "Beauty"
]

tables = [
    ("products", ["id", "name", "price"]),
    ("product", ["id", "name", "price", "category", "stock"]),
    ("productinfo", ["id", "name", "price", "category", "stock"]),
]


def random_name(length=20):
    return ''.join(random.choices(string.ascii_letters, k=length))


sock = socket.create_connection((HOST, PORT))

start = time.perf_counter()

inserted = 0

while inserted < TOTAL_ROWS:

    queries = []

    for _ in range(BATCH_SIZE):

        if inserted >= TOTAL_ROWS:
            break

        # Pick a random table
        table, columns = random.choice(tables)
        
        if table == "products":
            name = random_name()
            price = round(random.uniform(10, 5000), 2)
            queries.append(
                f"INSERT INTO {table} VALUES (1, '{name}', {price})"
            )
        else:
            name = random_name()
            price = round(random.uniform(10, 5000), 2)
            category = random.choice(categories)
            stock = random.randint(0, 1000)
            queries.append(
                f"INSERT INTO {table} VALUES (1, '{name}', {price}, '{category}', {stock})"
            )

        inserted += 1

    sock.sendall(("\n".join(queries) + "\n").encode())

    # Wait for server acknowledgement if your protocol returns one.
    sock.recv(1024)

    if inserted % 100 == 0:
        elapsed = time.perf_counter() - start
        print(
            f"{inserted:,} rows inserted | "
            f"{inserted/elapsed:,.0f} rows/sec"
        )

end = time.perf_counter()

#sock.close()

print("\nFinished")
print(f"Rows      : {TOTAL_ROWS:,}")
print(f"Time      : {end-start:.2f} sec")
print(f"Rows/sec  : {TOTAL_ROWS/(end-start):,.0f}")