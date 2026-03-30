tmp_stderr=$(mktemp)
conjure-oxide solve model.eprime model.param 2>"$tmp_stderr"
status=$?
sed -E "s/thread 'main' \\([0-9]+\\)/thread 'main' (THREAD_ID)/" "$tmp_stderr" >&2
rm -f "$tmp_stderr"
exit "$status"
