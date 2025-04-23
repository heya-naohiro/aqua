import subprocess
import signal
# import pytest
import os
import sys
ROOT = os.path.dirname(__file__)
INTEROP_DIR = os.path.join(ROOT, "paho_mqtt_testing", "interoperability")
sys.path.insert(0, os.path.abspath(INTEROP_DIR))
from paho_mqtt_testing.interoperability import client_test, client_test5

