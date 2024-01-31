This is a little Discord bot that I made mostly for the purpose of learning how the Discord API works. I tried to ensure that the developer ergonomics were pretty good too, for ease of local development.

# Flow

As the [official docs](https://discord.com/developers/docs/interactions/receiving-and-responding#receiving-an-interaction) detail, there are two ways to respond to a command. Since I want this program to be as simple as possible, we are creating an "outgoing webhook", which is essentially a server that discord will send incoming application commands to.

The flow is as follows:

1. Use the Discord API to owerwrite application commands ([Discord docs](https://discord.com/developers/docs/interactions/application-commands#bulk-overwrite-global-application-commands)).
1. Start the server.
1. Using the Discord API, edit the application to set the "interactions endpoint URL" to the value of the environment variable `INTERACTIONS_ENDPOINT_URL`.

# Authenticating

The outgoing Discord API requests are authenticated using a bot key that is read by the program using the environment variable `BOT_KEY`.
