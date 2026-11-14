import http.server
import json
import datetime
import sys

class PulseHandler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        data = {
            "gate": "northGate",
            "status": "ALIVE", 
            "role": "HEART",
            "color": "RED",
            "ts": datetime.datetime.utcnow().isoformat() + "Z",
            "gpu": "RTX 5090 32GB",
            "kenshi": "RUNNING",
            "meitou": "FORGING wave 251",
            "ollama_models": 63
        }
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        self.wfile.write(json.dumps(data).encode())
    def log_message(self, format, *args):
        pass

server = http.server.HTTPServer(("0.0.0.0", 8443), PulseHandler)
print("northGate RED EYE PULSE on :8443", flush=True)
server.serve_forever()
