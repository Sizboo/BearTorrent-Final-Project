c# 1. build docker:
docker build -t gcr.io/peerless-bond-456618-r8/server_image:latest .

# 2. push image to container registry
docker push gcr.io/peerless-bond-456618-r8/server_image:latest

# 3. deploy project
gcloud run deploy helpful-serf-server --image=gcr.io/peerless-bond-456618-r8/server_image:latest --region=us-south1 --platform=managed --allow-unauthenticated --use-http2

# url: https://helpful-serf-server-1016068426296.us-south1.run.app