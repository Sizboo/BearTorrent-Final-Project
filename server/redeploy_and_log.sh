
gcloud run deploy helpful-serf-server --image=gcr.io/peerless-bond-456618-r8/server_image:latest --region=us-south1 --platform=managed --allow-unauthenticated --use-http2

gcloud beta run services logs tail helpful-serf-server \
  --project=peerless-bond-456618-r8 \
  --region=us-south1 \
  --verbosity=debug