_calc()
{
    local cur prev words cword comp_args
    _comp_initialize -n =: -- "$@" || return

    local output status
    output=$(command calc --complete -- "${words[@]:1:cword}" 2>/dev/null)
    status=$?

    local IFS=$'\n'
    COMPREPLY=($(cut -f1 <<<"$output"))
    if ((status == 5)); then
        _comp_compgen -a filedir
    fi
    if [[ ${#COMPREPLY[@]} -eq 1 && ${COMPREPLY[0]} == *= ]]; then
        compopt -o nospace
    fi
}
complete -F _calc calc
