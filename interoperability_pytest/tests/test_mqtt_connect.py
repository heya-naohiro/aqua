import uuid
import pytest
import paho.mqtt.client as mqtt
import time
# from paho_test.interoperability import MQTTV311_spec
# from paho_test.interoperability import MQTTV311_spec

class TestMqttConnect:
    def setup_method(self):
        self.connected = False
        self.recieved_messages = []
        random_id = f"client-{uuid.uuid4()}"
        self.client = mqtt.Client(client_id=random_id)
        self.client.on_connect = self.on_connect
        self.client.on_message = self.on_message
        self.client.on_disconnect = self.on_disconnect

    def on_message(self, client, userdata, msg):
        self.recieved_messages.append(msg.payload.decode())
    
    def teardown_method(self):
        self.client.disconnect()
        self.client.loop_stop()
    def on_connect(self, client, userdata, flags, rc):
        if rc == 0:
            self.connected = True
        else:
            print(f"MQTT connection failed with code {rc}")
    def on_disconnect(self, client, userdata, rc):
        print(f"Disconnected with result code {rc}")

    """
    def test_connect(self):
        self.client.connect("127.0.0.1", 1883, 60)
        self.client.loop_start()

        timeout = time.time() + 5  # 5秒間待機
        while not self.connected and time.time() < timeout:
            time.sleep(0.1)

        assert self.connected, "MQTT failed to connect"
    """
    
    def test_subscribe_recieve(self):
        self.client.connect("127.0.0.1", 1883, 60)
        self.client.loop_start()

        for _ in range(10):
            if self.connected:
                break
            time.sleep(0.1)
        assert self.connected, "MQTT failed"

        print(" ^^^^^^^ subscribe ^^^^^^")
        self.client.subscribe("test/topic")
        time.sleep(1)
        print(" ^^^^^^^ subscribe done ^^^^^^")

        print(" ^^^^^^^ publish ^^^^^^")
        self.client.publish("test/topic", "Hello MQTT")
        time.sleep(10)
        print(" ^^^^^^^ publish done ^^^^^^")

        assert "Hello MQTT" in self.recieved_messages, "Subscribe でメッセージを受信できませんでした"