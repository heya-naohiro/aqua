import subprocess
import time
import pytest

@pytest.fixture(scope="session", autouse=True)
def start_mqtt_example_server():
    proc = subprocess.Popen(["cargo", "run", "--example", "broker"])
    time.sleep(10)
    yield
    proc.terminate()
    proc.wait()
