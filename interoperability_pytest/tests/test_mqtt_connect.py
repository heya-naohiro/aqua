import pytest
import paho.mqtt.client as mqtt
import time
# from paho_test.interoperability import MQTTV311_spec
# from paho_test.interoperability import MQTTV311_spec

class TestMqttConnect:
    def setup_method(self):
        self.connected = False
        self.recieved_messages = []

        self.client = mqtt.Client(client_id="helloclient")
        self.client.on_connect = self.on_connect
        self.client.on_message = self.on_message
    
    def on_message(self, client, userdata, msg):
        self.recieved_messages.append(msg.payload.decode())
    
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
        time.sleep(0.5)
        print(" ^^^^^^^ publish done ^^^^^^")

        assert "Hello MQTT" in self.recieved_messages, "Subscribe でメッセージを受信できませんでした"