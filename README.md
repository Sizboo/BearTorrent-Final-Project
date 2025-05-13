[![Review Assignment Due Date](https://classroom.github.com/assets/deadline-readme-button-22041afd0340ce965d47ae6ef1cefeee28c7c493a6346c4f15d667ab976d596c.svg)](https://classroom.github.com/a/6FRwiRqU)
Goal: Apply the knowledge you've learned in new ways.

# Project description
This is an open-ended project. Students can extend their BearTV project or do something new from the ground up. Project ideas must be approved by Dr. Freeman.

You must give a **formal presentation** of your project in place of a final exam. Each group will have ~12 minutes to present their work. Each member of the group must speak. You should have slides. Your presentation must include a demo of your project, although it may invlude a pre-recorded screen capture. In your presentation, you should introduce the problem that you addressed, how you addressed it, technical challenges you faced, what you learned, and next steps (if you were to continue developing it).

You may use AI LLM tools to assist with the development of your project, including code assistant tools like GitHub Copilot. If you do use any AI tools, you must describe your use during your presentation.

Unless you get specific approval otherwise, your project **must** include some component deployed on a cloud hosting service. You can use AWS, GCP, Azure, etc. These services have free tiers, and you might consider looking into tiers specifically for students.

## Milestones
- You must meet with Dr. Freeman within the first week to get your project idea approved
- You must meet with Dr. Freeman within the first 3 weeks to give a status update and discuss roadblocks
- See the course schedule spreadhseet for specific dates

## Project Ideas
- Simulate UDP packet loss and packet corruption in BearTV in a non-deterministic way (i.e., don't just drop every Nth packet). Then, extend the application protocol to be able to detect and handle this packet loss.
- Extend the BearTV protocol to support streaming images (or video!) alongside the CC data, and visually display them on the client. This should be done in such a way that it is safely deliver*able* over *any* implementation of IPv4. The images don't have to be relevant to the caption data--you can get them randomly on the server from some image source.
- Do something hands on with a video streaming protocol such as MoQ, DASH, or HLS.
- Implement QUIC
- Develop a new congestion control algorithm and evaluate it compared to existing algorithms in a realistic setting
- Make significant contributions to a relevant open-source repository (e.g., moq-rs)
- Implement a VPN
- Implement a DNS
- Do something with route optimization
- Implement an HTTP protocol and have a simple website demo

--> These are just examples. I hope that you'll come up with a better idea to suit your own interests!

## Libraries

Depending on the project, there may be helpful libraries you find to help you out. However, there may also be libraries that do all the interesting work for you. Depending on the project, you'll need to determine what should be fair game. For example, if your project is to implement HTTP, then you shouldn't leverage an HTTP library that does it for you.

If you're unsure if a library is okay to use, just ask me.

## Languages

The core of your project should, ideally, be written in Rust. Depending on the project idea, however, I'm open to allowing the use of other languages if there's a good reason for it. For me to approve such a request, the use of a different language should enable greater learning opportunities for your group.

# Submission

## Setup Instructions
- Use rust version 1.86.0 (what we used)
- Make sure protoc (Protocol Buffers compiler) is installed. We used this to generate proto files for use with gRPC via Tonic.
	- Run a command such as '''sudo pacman -S protobuf''' (or whatever package manager you use.)
 - 
## FRONTEND 
Install Tauri Into Client/
Npm run build inside Client/frontend
cargo tauri dev inside Client/

## Questions
- What is your project?
	- Our project is a torrent client that implements our own version of the BitTorrent protocol. It supports file sharing across networks via peer-to-peer connections.
- What novel work did you do?
	- We've created a program that allows for advertising local files and downloading foreign files from other peers using the client. We support different types of client connections through LAN, P2P via UDP holepunching, and TURN as a last resort. We offer high-speed downloading of large files from multiple peers at the same time.   
- What did you learn?
	- We learned how to connect clients across different networks, advertise local files to other clients, and download from multiple seeders. We also got experience with gRPC and gCloud-run servers, and rust of course.
- What was challenging?
	- Figuring out how to connect to clients on different networks was challenging, especially with the different NAT types. UDP Hole-punching was especially difficult.
- What AI tools did you use, and what did you use them for? What were their benefits and drawbacks?
	- We used ChatGPT to assist with concepting code and debugging errors within our program. 
- What would you do differently next time?
	- On the server side, we would swap out the usage of maps with a different data structure to mitigate the potential of deadlock. 

## What to submit
- Push your working code to the main branch of your team's GitHub Repository before the deadline
- Edit the README to answer the above questions
- On Teams, *each* member of the group must individually upload answers to these questions:
	- What did you (as an individual) contribute to this project?
	- What did the other members of your team contribute?
	- Do you have any concerns about your own performance or that of your team members? Any comments will remain confidential, and Dr. Freeman will try to address them in a way that preserves anonymity.
	- What feedback do you have about this course?

## Grading

Grading will be based on...
- The technical merit of the group's project
- The contribution of each individual group member
- Evidence of consistent work, as revealed during milestone meetings
- The quality of the final presentation
