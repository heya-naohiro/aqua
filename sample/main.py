import paho.mqtt.client as mqtt
import time
from paho.mqtt.properties import Properties
from paho.mqtt.packettypes import PacketTypes
from paho.mqtt.reasoncodes import ReasonCode  # ✅ 単数形を使用

broker = "localhost"
port = 1883
topic = "test/topic"

client = mqtt.Client(protocol=mqtt.MQTTv5)

def on_connect(client, userdata, flags, rc, properties=None):
    print("Connected with result code " + str(rc))

def on_publish(client, userdata, mid):
    print("Message published with mid: " + str(mid))

client.on_connect = on_connect
client.on_publish = on_publish

client.connect(broker, port, keepalive=60)
client.loop_start()

result = client.publish(topic, payload="Hello MQTT QoS 2", qos=2)
result.wait_for_publish()
time.sleep(1)

# DISCONNECT プロパティの設定（任意）
disconnect_properties = Properties(PacketTypes.DISCONNECT)
disconnect_properties.UserProperty = [("reason", "shutdown")]
disconnect_properties.SessionExpiryInterval = 999
disconnect_properties.ServerReference = "your.server.address" 
# ✅ ReasonCode を使って正常切断
client.disconnect(
    reasoncode=ReasonCode(PacketTypes.DISCONNECT, "Normal disconnection"),
    properties=disconnect_properties
)

client.loop_stop()