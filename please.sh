# please — describe what you want, Claude writes the shell command, you confirm, it runs.
# Usage:  please compress the video in this folder
# Works in bash and zsh. Override the model with: export PLEASE_MODEL=sonnet
please() {
  if [ $# -eq 0 ]; then
    echo "usage: please <what you want to do>" >&2
    return 1
  fi

  local req="$*"
  local ctx
  ctx="OS: $(uname -s) $(uname -r) $(uname -m)
Shell: $(basename "${SHELL:-sh}")
Current directory: $PWD
Directory listing (ls -la, first 60 lines):
$(ls -la 2>/dev/null | head -60)"

  printf '\033[2m…thinking\033[0m\r' >&2
  local cmd
  cmd=$(claude -p --model "${PLEASE_MODEL:-haiku}" \
    --system-prompt "You turn a natural-language request into exactly ONE shell command for the user's machine. Output ONLY the raw command: no markdown, no code fences, no comments, no explanation. If several steps are needed, chain them with && or use a one-line loop. Use the real file names from the directory listing. Prefer tools commonly available on this OS. Do not use sudo unless the request truly needs it." \
    "$ctx

Request: $req" </dev/null 2>/dev/null)
  printf '\033[K' >&2

  # strip code fences / blank lines / surrounding whitespace, just in case
  cmd=$(printf '%s\n' "$cmd" | sed -e '/^```/d' -e '/^[[:space:]]*$/d')
  cmd="${cmd#"${cmd%%[![:space:]]*}"}"
  cmd="${cmd%"${cmd##*[![:space:]]}"}"

  if [ -z "$cmd" ]; then
    echo "please: no command came back (is 'claude' installed and logged in?)" >&2
    return 1
  fi

  printf '\n  \033[1;36m%s\033[0m\n\n' "$cmd"
  printf 'Run it? [Y/n/e=edit] '
  local ans
  read -r ans
  case "$ans" in
    n|N|no|NO) echo "cancelled"; return 1 ;;
    e|E)
      if [ -n "$ZSH_VERSION" ]; then
        vared -p '> ' cmd
      else
        read -e -r -p '> ' -i "$cmd" cmd
      fi
      [ -z "$cmd" ] && { echo "cancelled"; return 1; }
      ;;
  esac

  # put the real command into shell history so ↑ recalls it
  if [ -n "$ZSH_VERSION" ]; then print -s -- "$cmd"; else history -s -- "$cmd"; fi

  eval "$cmd"
}
