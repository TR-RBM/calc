function __calc_complete
    set -l words (commandline -xpc)[2..-1] (commandline -ct)
    command calc --complete -- $words
    if test $status -eq 5
        __fish_complete_path (commandline -ct)
    end
end

complete -c calc -f -a '(__calc_complete)'
