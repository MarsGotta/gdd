# When git cannot add your worktree

Use your own worktree inside the project. If git cannot add it, clone the project in your own temporary folder. Copy the project's `user.name` and `user.email` into the clone's git config. Read each value in the project with `git config --get user.name` or `git config --get user.email` and set it in the clone with `git config --local`.

Start from the project's current branch tip when resuming a role. Bring the branch back before reporting: from the project run `git fetch <clone> <branch>:<branch>`. If git refuses the fetch, stop with `CANNOT:`, naming the branch and the file you were working on. Never force it.

For example:

    CANNOT: Git refused to fetch work/labels while I was editing labels.txt.
    I need that branch free in the project to bring the commit back.
