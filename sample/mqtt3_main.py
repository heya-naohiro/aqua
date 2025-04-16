import time
import paho.mqtt.client as mqtt

# ===============================
# 設定
# ===============================
BROKER = "localhost"
PORT = 1883

# ===============================
# サブスクライブ対象のトピックとQoS
# ===============================
TOPIC_QOS_LIST = [
    ("topic/test/qos0", 0),
    ("topic/test/qos1", 1),
    ("topic/test/qos2", 2),
]

# ===============================
# コールバック定義
# ===============================
def on_connect(client, userdata, flags, rc):
    print(f"[on_connect] Connected with result code: {rc}")

    for topic, qos in TOPIC_QOS_LIST:
        print(f"Subscribing to: {topic} with QoS: {qos}")
        client.subscribe(topic, qos)

def on_subscribe(client, userdata, mid, granted_qos):
    print(f"[on_subscribe] MID: {mid} | Granted QoS: {granted_qos}")

def on_message(client, userdata, msg):
    print(f"[on_message] Topic: {msg.topic} | QoS: {msg.qos} | Payload: {msg.payload.decode()}")

# ===============================
# クライアント作成（MQTT v3.1.1）
# ===============================
client = mqtt.Client(client_id="mqtt-v3-subscriber", protocol=mqtt.MQTTv311)

# コールバック登録
client.on_connect = on_connect
client.on_subscribe = on_subscribe
client.on_message = on_message

client.connect(BROKER, PORT, keepalive=60)
client.loop_start()

try:
    while True:
        time.sleep(5)
except KeyboardInterrupt:
    print("Interrupt received. Unsubscribing from topics and disconnecting...")

    # トピック名だけをリストにしてまとめて Unsubscribe
    unsubscribe_topics = [topic for topic, _ in TOPIC_QOS_LIST]
    client.unsubscribe(unsubscribe_topics)

    client.loop_stop()
    client.disconnect()
