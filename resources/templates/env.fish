# rim shell setup (inspired by rustup)
# DO NOT modify
function add_to_path
    set path_to_add $argv[1]

    if not contains "$path_to_add" $PATH
        set -x PATH "$path_to_add" $PATH
    end
end
