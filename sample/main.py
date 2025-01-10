from paho.mqtt import client as mqtt_client
from paho.mqtt import properties;
from paho.mqtt import packettypes;
import random

broker = '127.0.0.1'
port = 1883
topic = "python/mqtt"
client_id = f'publish-{random.randint(0, 1000)}'

def on_connect(client, userdata, flags, rc):
    if rc == 0:
        print("Connected to MQTT Broker!")
    else:
        print("Failed to connect, return code %d\n", rc)

def run():
    client = mqtt_client.Client(mqtt_client.CallbackAPIVersion.VERSION1, 
                                client_id=client_id,
                                protocol=mqtt_client.MQTTProtocolVersion.MQTTv5)
    client.username_pw_set(username="your_username", password="your_password")
    # client.on_connect = on_connect
    # Willメッセージの設定
    # Willメッセージのプロパティ設定
    will_properties = properties.Properties(packettypes.PacketTypes.WILLMESSAGE)
    will_properties.WillDelayInterval = 10  # Willメッセージが遅延される秒数
    will_properties.MessageExpiryInterval = 30  # メッセージが期限切れになる秒数
    will_properties.ContentType = "text/plain"  # メッセージの内容のタイプ
    will_properties.PayloadFormatIndicator = 1  # ペイロードがUTF-8エンコードされた文字列
    will_properties.ResponseTopic = "test/response"  # 応答を受け取るためのトピック
    will_properties.CorrelationData = b"correlation_data_example"  # バイナリデータ 24bytes=0x18
    will_properties.UserProperty = [
        ("key1", "value1"),
        ("key2", "value2"),
        ("key3", "value3")
    ]  # 複数のユーザープロパティを設定

    client.will_set(
        topic="test/will",
        payload="Will message",
        qos=1,
        retain=False,
        properties=will_properties
    )


    connect_properties = properties.Properties(properties.PacketTypes.CONNECT)
    connect_properties.SessionExpiryInterval = 120  # セッションの有効期限 (秒)
    connect_properties.ReceiveMaximum = 20  # 同時に受信可能なQoS 1またはQoS 2メッセージの最大数
    connect_properties.MaximumPacketSize = 256 * 1024  # 最大パケットサイズ (バイト)
    connect_properties.TopicAliasMaximum = 10  # サポートするトピックエイリアスの最大数
    connect_properties.RequestResponseInformation = 1  # サーバーから応答トピックを要求するかどうか
    connect_properties.RequestProblemInformation = 1  # 問題情報を要求するかどうか
    connect_properties.UserProperty = [
        ("connect_key1", "connect_value1"),
        ("connect_key2", "connect_value2"),
    ]
    connect_properties.AuthenticationMethod = "example_auth_method"  # 認証に使用するメソッド
    connect_properties.AuthenticationData = b"example_auth_data"  # 認証に関連するデータ

    client.connect(broker, port, properties=connect_properties)

    publish_properties = properties.Properties(properties.PacketTypes.PUBLISH)
    publish_properties.UserProperty = ("a", "2")
    publish_properties.UserProperty = ("c", "3")
    publish_properties.MessageExpiryInterval = 60  # メッセージの有効期限（秒）
    publish_properties.PayloadFormatIndicator = 1  # ペイロードがUTF-8形式であることを示す
    publish_properties.ContentType = "text/plain"
    publish_properties.ResponseTopic = "response/topic"
    publish_properties.CorrelationData = b"12345"
    client.publish(topic="hello/topic", payload="payload", qos=1, retain=True, properties=publish_properties)

if __name__ == '__main__':
    run()
    
    