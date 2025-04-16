import time
import paho.mqtt.client as mqtt
from paho.mqtt.properties import Properties
from paho.mqtt.packettypes import PacketTypes
from paho.mqtt.subscribeoptions import SubscribeOptions

# ===============================
# 設定
# ===============================
BROKER = "localhost"
PORT = 1883

# ===============================
# Subscribe 時に指定する Subscription Options
# ===============================
TOPIC_OPTIONS = [
    # QoS 0, retain_handling = always, no_local = False
    ("topic/test/qos0", SubscribeOptions(qos=0, noLocal=False, retainAsPublished=False, retainHandling=0)),

    # QoS 1, retain_handling = only if not existing, no_local = True
    ("topic/test/qos1", SubscribeOptions(qos=1, noLocal=True, retainAsPublished=True, retainHandling=1)),

    # QoS 2, retain_handling = never send retained, no_local = False
    ("topic/test/qos2", SubscribeOptions(qos=2, noLocal=False, retainAsPublished=False, retainHandling=2)),
]

# ===============================
# コールバック定義
# ===============================
def on_connect(client, userdata, flags, reason_code, properties):
    print(f"[on_connect] Connected with reason code: {reason_code}")

    for i, (topic, options) in enumerate(TOPIC_OPTIONS):
        print(f"Subscribing to: {topic} with options: {options}")

        # SUBSCRIBE プロパティを構築
        sub_props = Properties(PacketTypes.SUBSCRIBE)
        sub_props.SubscriptionIdentifier = i + 1
        sub_props.UserProperty = [("source", "test-script"), ("topic-index", str(i))]

        client.subscribe(topic, options=options, properties=sub_props)

def on_subscribe(client, userdata, mid, granted_qos, properties):
    print(f"[on_subscribe] MID: {mid} | Granted QoS: {granted_qos}")
    if properties:
        print(f"  Properties: {properties}")

def on_message(client, userdata, msg):
    print(f"[on_message] Topic: {msg.topic} | QoS: {msg.qos} | Payload: {msg.payload.decode()}")

# ===============================
# クライアント作成
# ===============================
client = mqtt.Client(client_id="mqtt-v5-subscriber", protocol=mqtt.MQTTv5)

# Clean Start 相当のプロパティ
connect_props = Properties(PacketTypes.CONNECT)
connect_props.SessionExpiryInterval = 0  # 即時セッション破棄

# コールバック登録
client.on_connect = on_connect
client.on_subscribe = on_subscribe
client.on_message = on_message

client.connect(BROKER, PORT, keepalive=60, properties=connect_props)
client.loop_start()

try:
    while True:
        time.sleep(5)
except KeyboardInterrupt:
    print("Interrupt received. Unsubscribing from topics and disconnecting...")
    # 各トピックからUnsubscribeするシーケンス
    # トピック名だけのリストを作る
    unsubscribe_topics = [topic for topic, _ in TOPIC_OPTIONS]

    # プロパティを一つにまとめて送信
    unsub_props = Properties(PacketTypes.UNSUBSCRIBE)
    unsub_props.UserProperty = [("source", "test-script"), ("unsubscribe", "batch")]

    print(f"Unsubscribing from topics: {unsubscribe_topics}")
    client.unsubscribe(unsubscribe_topics, properties=unsub_props)

    client.loop_stop()
    client.disconnect()
