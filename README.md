# cherries-4-prs

My employer, as of the time of this writing, has an "employee benefit" program
with [Bonusly], where each month Bonusly gives each employee 50 "cherries" to
give away to other users (cherries you've been gifted can be redeemed for
various rewards at a rate of about $0.10/cherry). At the end of the month, any
cherries you haven't given away dissapear.

I always forget to give away my cherries, so I've built this automation to
automatically send my coworkers cherries when they approve my PRs for merge.
(Cherries are sent once per reviewer per PR.)

See the `config.toml` file for configuration options, and
`cherries-4-prs.service` for an example systemd unit file to run cherries-4-prs
in the background.

[Bonusly]: https://bonus.ly/
