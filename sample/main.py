from paho.mqtt import client as mqtt_client
import random

broker = '192.168.0.100'
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
    # client.on_connect = on_connect
    # client.will_set()
 #   properies = mqtt_client.Properties()
 #   connect_properties = mqtt_client.Properties(mqtt_client.PacketTypes.CONNECT)
 #   connect_properties.
    client.connect(broker, port)

if __name__ == '__main__':
    run()