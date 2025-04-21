import paho.mqtt.client as mqtt
import time

# MQTTブローカー設定（パブリックなブローカー例）
broker = "localhost"
port = 1883
topic = "test/topic"

# クライアント作成
client = mqtt.Client()

# 接続成功時のコールバック
def on_connect(client, userdata, flags, rc):
    print("Connected with result code " + str(rc))

# Publish完了時のコールバック（QoS 1または2で呼ばれる）
def on_publish(client, userdata, mid):
    print("Message published with mid: " + str(mid))

client.on_connect = on_connect
client.on_publish = on_publish

# ブローカーに接続
client.connect(broker, port, keepalive=60)

# ループを開始（非同期）
client.loop_start()

# QoS 2 で Publish
result = client.publish(topic, payload="Hello MQTT QoS 2", qos=2)

# QoS 2 では4-way handshake（PUBLISH, PUBREC, PUBREL, PUBCOMP）が行われる
# 完了を明示的に待つ場合は wait_for_publish() を使う
result.wait_for_publish()

# 少し待機して、非同期処理を完了させる
time.sleep(1)

# 終了処理
client.loop_stop()
client.disconnect()