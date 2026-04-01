# Sharing Agents

Built a useful agent? Share it with the community by opening a pull request.

## How sharing works

Community agents live in the [`examples/agents/`](https://github.com/undivisible/poke-around/tree/main/examples/agents) directory of the Poke Around repository. Anyone can install them with:

```bash
./poke-around agent get <name>
```

## Submit your agent

### 1. Fork the repo

Go to [github.com/undivisible/poke-around](https://github.com/undivisible/poke-around) and click **Fork**.

### 2. Add your agent files

Place your files in `examples/agents/`:

```
examples/agents/
  your-agent.1h.js      # The agent script
  .env.your-agent       # Env template with placeholder values
```

### 3. Agent checklist

Before submitting, make sure your agent:

- Has a **frontmatter block** with `@agent`, `@name`, `@description`, `@interval`, `@env`, `@author`
- Uses **placeholder values** in the `.env` file (e.g. `YOUR_TOKEN_HERE`) — never real credentials
- Has **comments in the `.env`** explaining where to find each value
- Handles **errors gracefully** — logs useful messages, doesn't crash silently
- Stays within the **5-minute timeout**
- Uses `getToken()` from the Poke SDK for authentication (not hardcoded tokens)

### 4. Frontmatter example

```javascript
/**
 * @agent your-agent
 * @name Your Agent Name
 * @description Clear one-line description of what this agent does.
 * @interval 1h
 * @env API_TOKEN - Where to find this token
 * @env BASE_URL - (optional) Override the default URL
 * @author your-github-username
 */
```

### 5. Env template example

```env
# Where to find this token: App > Settings > API
API_TOKEN=your_token_here

# Optional: override the default API URL
# BASE_URL=http://localhost:8080
```

Use `your_*_here` as placeholder values — the installer detects these and prompts the user.

### 6. Open a PR

Push to your fork and open a pull request to `undivisible/poke-around` with:

- **Title:** `agent: add <name>`
- **Description:** What the agent does, what service it connects to, any prerequisites

```bash
git checkout -b agent/your-agent
git add examples/agents/
git commit -m "agent: add your-agent"
git push origin agent/your-agent
```

Then open the PR at [github.com/undivisible/poke-around/compare](https://github.com/undivisible/poke-around/compare).

## Agent ideas

Looking for inspiration? Here are some agents the community would love:

| Idea | What it does |
|------|-------------|
| **GitHub notifications** | Fetch unread notifications and send a digest |
| **Calendar summary** | Summarize today's upcoming meetings |
| **Disk space monitor** | Alert when disk space is below a threshold |
| **Uptime checker** | Ping a list of URLs and report any downtime |
| **RSS reader** | Fetch latest articles from your feeds |
| **Git status** | Report uncommitted changes across your projects |
| **Docker health** | Check running containers and their status |
| **Mail digest** | Summarize unread emails from a local mail client |

Build one and share it!
