import pytest
import paho.mqtt.client as mqtt
import time
# from paho_test.interoperability import MQTTV311_spec
# from paho_test.interoperability import MQTTV311_spec

class TestMqttConnect:
    def setup_method(self):
        self.connected = False
        self.client = mqtt.Client()
        self.client.on_connect = self.on_connect
    
    def teardown_method(self):
        self.client.disconnect()
        self.client.loop_stop()
    def on_connect(self, client, userdata, flags, rc):
        if rc == 0:
            self.connected = True

    def test_connect(self):
        self.client.connect("127.0.0.1", 1883, 60)
        self.client.loop_start()

        for _ in range(10):
            if self.connected:
                break

            time.sleep(0.1)
        
        assert self.connected, "MQTT failed"